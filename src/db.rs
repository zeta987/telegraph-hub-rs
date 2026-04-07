use std::path::Path;

use rusqlite::{Connection, params};

use crate::cache::PageSummary;

/// A single token's persisted cache entry loaded from SQLite.
pub struct CacheEntry {
    pub token_hash: String,
    pub pages: Vec<PageSummary>,
    pub total_count: i64,
    pub created_at_unix: i64,
}

/// SQLite-backed persistence layer for page cache data.
///
/// Stores page summaries keyed by token hash, with metadata
/// (total count, creation timestamp) for TTL enforcement.
pub struct Database {
    conn: Connection,
}

/// Pre-create `path` with owner-only permissions (0600) on Unix.
///
/// Uses `OpenOptions::create_new` (which maps to `O_CREAT | O_EXCL` on
/// the libc layer) so the create-or-skip decision is a single atomic
/// syscall — there is no window between an `exists()` check and the
/// open() call during which a concurrent process could race in and
/// create a world-readable file. If the file already exists, the
/// `AlreadyExists` error is swallowed and the caller is responsible
/// for inspecting the existing mode separately (see
/// [`detect_loose_permissions`]).
///
/// On non-Unix platforms this emits a single process-wide `info`
/// message (gated by `std::sync::Once`) and returns `Ok(())`, so
/// operators see a clear statement that no enforcement is applied
/// and should use directory ACLs or a user-profile path.
fn ensure_file_with_restricted_mode(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::ErrorKind;
        use std::os::unix::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
        {
            // Atomically created with mode 0600; drop the handle so
            // rusqlite can re-open the file through its normal path.
            Ok(_file) => Ok(()),
            // Pre-existing file is not an error — the post-open
            // `detect_loose_permissions` check handles loose modes.
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e),
        }
    }
    #[cfg(not(unix))]
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            tracing::info!(
                "Cache database file permission enforcement is not applied on this platform; \
                 restrict access via directory ACLs or place the file under a user-profile path"
            );
        });
        let _ = path;
        Ok(())
    }
}

/// Report the octal permission bits of `path` when they are more
/// permissive than `0o600` on Unix.
///
/// Returns `None` when permissions are already owner-only, when the
/// path cannot be stat'd, or on non-Unix platforms. Extracted as a
/// pure function so unit tests can assert the detection predicate
/// directly without capturing tracing output.
fn detect_loose_permissions(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path).ok()?;
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 { Some(mode) } else { None }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

impl Database {
    /// Open (or create) the SQLite database at `path` and ensure schema exists.
    ///
    /// On Unix, the cache file is atomically created with mode `0600` before
    /// SQLite opens it, eliminating any window of world-readability. If the
    /// file already exists with looser permissions, a warning is logged but
    /// the file is left untouched so operators who intentionally share it
    /// are not surprised. On non-Unix platforms, no automated enforcement is
    /// applied and operators are expected to restrict access via directory
    /// ACLs.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        // SQLite's in-memory sentinel; skip filesystem hardening entirely.
        // Production callers pass a filesystem path from TELEGRAPH_HUB_DB;
        // this branch is only hit by unit tests in this module.
        let is_memory = path.to_str() == Some(":memory:");

        if !is_memory && let Err(e) = ensure_file_with_restricted_mode(path) {
            tracing::warn!(
                "Failed to pre-create cache database file {} with 0600 mode: {e}. \
                 Continuing with default open permissions.",
                path.display()
            );
        }

        let conn = Connection::open(path)?;

        if !is_memory && let Some(mode) = detect_loose_permissions(path) {
            tracing::warn!(
                "Cache database file {} has permissive mode {:o} (expected 600). \
                 telegraph-hub-rs does not forcibly change it; restrict access manually.",
                path.display(),
                mode
            );
        }

        // Enable WAL mode for better concurrent read performance
        conn.pragma_update(None, "journal_mode", "WAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS page_cache (
                token_hash TEXT NOT NULL,
                path       TEXT NOT NULL,
                title      TEXT NOT NULL,
                url        TEXT NOT NULL,
                views      INTEGER NOT NULL,
                PRIMARY KEY (token_hash, path)
            );
            CREATE TABLE IF NOT EXISTS cache_meta (
                token_hash      TEXT PRIMARY KEY,
                total_count     INTEGER NOT NULL,
                created_at_unix INTEGER NOT NULL
            );",
        )?;

        Ok(Self { conn })
    }

    /// Load all cached token entries from the database.
    pub fn load_all(&self) -> Result<Vec<CacheEntry>, rusqlite::Error> {
        let mut meta_stmt = self
            .conn
            .prepare("SELECT token_hash, total_count, created_at_unix FROM cache_meta")?;

        let metas: Vec<(String, i64, i64)> = meta_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<_, _>>()?;

        let mut page_stmt = self
            .conn
            .prepare("SELECT path, title, url, views FROM page_cache WHERE token_hash = ?")?;

        let mut results = Vec::with_capacity(metas.len());
        for (token_hash, total_count, created_at_unix) in metas {
            let pages: Vec<PageSummary> = page_stmt
                .query_map(params![token_hash], |row| {
                    Ok(PageSummary {
                        path: row.get(0)?,
                        title: row.get(1)?,
                        url: row.get(2)?,
                        views: row.get(3)?,
                    })
                })?
                .collect::<Result<_, _>>()?;

            results.push(CacheEntry {
                token_hash,
                pages,
                total_count,
                created_at_unix,
            });
        }

        Ok(results)
    }

    /// Persist a fully-built page cache for a given token hash.
    ///
    /// Replaces any existing data for `token_hash` within a single transaction.
    pub fn save(
        &mut self,
        token_hash: &str,
        pages: &[PageSummary],
        total_count: i64,
    ) -> Result<(), rusqlite::Error> {
        let tx = self.conn.transaction()?;

        tx.execute(
            "DELETE FROM page_cache WHERE token_hash = ?",
            params![token_hash],
        )?;
        tx.execute(
            "DELETE FROM cache_meta WHERE token_hash = ?",
            params![token_hash],
        )?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO page_cache (token_hash, path, title, url, views) VALUES (?, ?, ?, ?, ?)",
            )?;
            for page in pages {
                stmt.execute(params![
                    token_hash, page.path, page.title, page.url, page.views,
                ])?;
            }
        }

        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs() as i64;

        tx.execute(
            "INSERT INTO cache_meta (token_hash, total_count, created_at_unix) VALUES (?, ?, ?)",
            params![token_hash, total_count, now_unix],
        )?;

        tx.commit()
    }

    /// Mark specific pages as deleted in the cache (update title to `[DELETED]`).
    pub fn mark_deleted(&self, token_hash: &str, paths: &[String]) -> Result<(), rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "UPDATE page_cache SET title = '[DELETED]' WHERE token_hash = ? AND path = ?",
        )?;
        for path in paths {
            stmt.execute(params![token_hash, path])?;
        }
        Ok(())
    }

    /// Remove all cached data for a given token hash.
    pub fn invalidate(&self, token_hash: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM page_cache WHERE token_hash = ?",
            params![token_hash],
        )?;
        self.conn.execute(
            "DELETE FROM cache_meta WHERE token_hash = ?",
            params![token_hash],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_test_db() -> Database {
        Database::open(Path::new(":memory:")).expect("in-memory DB")
    }

    fn sample_pages() -> Vec<PageSummary> {
        vec![
            PageSummary {
                path: "test-page-01".to_string(),
                title: "Test Page 1".to_string(),
                url: "https://telegra.ph/test-page-01".to_string(),
                views: 100,
            },
            PageSummary {
                path: "test-page-02".to_string(),
                title: "Test Page 2".to_string(),
                url: "https://telegra.ph/test-page-02".to_string(),
                views: 200,
            },
        ]
    }

    #[test]
    fn save_and_load_roundtrip() {
        let mut db = make_test_db();
        let pages = sample_pages();

        db.save("hash_abc", &pages, 2).unwrap();

        let loaded = db.load_all().unwrap();
        assert_eq!(loaded.len(), 1);

        let entry = &loaded[0];
        assert_eq!(entry.token_hash, "hash_abc");
        assert_eq!(entry.pages.len(), 2);
        assert_eq!(entry.total_count, 2);
        assert!(entry.created_at_unix > 0);
        assert_eq!(entry.pages[0].path, "test-page-01");
        assert_eq!(entry.pages[1].views, 200);
    }

    #[test]
    fn save_overwrites_existing() {
        let mut db = make_test_db();
        let pages = sample_pages();

        db.save("hash_abc", &pages, 2).unwrap();
        db.save(
            "hash_abc",
            &[PageSummary {
                path: "new-page".to_string(),
                title: "New".to_string(),
                url: "https://telegra.ph/new-page".to_string(),
                views: 42,
            }],
            1,
        )
        .unwrap();

        let loaded = db.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pages.len(), 1);
        assert_eq!(loaded[0].pages[0].path, "new-page");
        assert_eq!(loaded[0].total_count, 1);
    }

    #[test]
    fn invalidate_removes_data() {
        let mut db = make_test_db();
        db.save("hash_abc", &sample_pages(), 2).unwrap();

        db.invalidate("hash_abc").unwrap();

        let loaded = db.load_all().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_all_empty_db() {
        let db = make_test_db();
        let loaded = db.load_all().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn multiple_tokens() {
        let mut db = make_test_db();
        db.save("hash_a", &sample_pages(), 2).unwrap();
        db.save(
            "hash_b",
            &[PageSummary {
                path: "other".to_string(),
                title: "Other".to_string(),
                url: "https://telegra.ph/other".to_string(),
                views: 5,
            }],
            1,
        )
        .unwrap();

        let loaded = db.load_all().unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn created_at_unix_is_recent() {
        let mut db = make_test_db();
        db.save("hash_abc", &sample_pages(), 2).unwrap();

        let loaded = db.load_all().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ts = loaded[0].created_at_unix;

        // Timestamp should be within last 5 seconds
        assert!((now - ts).abs() < 5);
    }

    #[test]
    fn in_memory_database_does_not_touch_filesystem() {
        // `:memory:` is SQLite's in-memory sentinel. Opening it must not
        // consult the filesystem, must not call `ensure_file_with_restricted_mode`,
        // and must not emit a permission warning — all guaranteed by the
        // `is_memory` short-circuit inside `Database::open`.
        let db = Database::open(Path::new(":memory:")).expect("open :memory:");
        drop(db);
        assert!(!Path::new(":memory:").exists());
    }

    #[cfg(unix)]
    #[test]
    fn fresh_database_file_has_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db_path = tmp.path().join("fresh.db");
        assert!(!db_path.exists(), "precondition: path must not exist");

        let _db = Database::open(&db_path).expect("open");

        let mode = std::fs::metadata(&db_path)
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn existing_permissive_file_triggers_warning_log() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone, Default)]
        struct BufWriter(Arc<Mutex<Vec<u8>>>);
        struct BufGuard(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for BufGuard {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> MakeWriter<'a> for BufWriter {
            type Writer = BufGuard;
            fn make_writer(&'a self) -> Self::Writer {
                BufGuard(self.0.clone())
            }
        }

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let db_path = tmp.path().join("preexisting.db");

        // Pre-create the file with 0o644 so the warning branch fires.
        std::fs::File::create(&db_path).expect("create");
        std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644))
            .expect("chmod 644");

        // Install a thread-local tracing subscriber that writes into a
        // buffer. `set_default` (not `set_global_default`) scopes the
        // subscriber to the current thread via its returned guard, so
        // parallel tests cannot collide on a global tracing state.
        let buf = BufWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(buf.clone())
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .finish();
        let captured = {
            let _guard = tracing::subscriber::set_default(subscriber);
            let _db = Database::open(&db_path).expect("open");
            String::from_utf8(buf.0.lock().unwrap().clone()).expect("utf8")
        };

        // The warning MUST be emitted and MUST name mode 644.
        assert!(
            captured.contains("permissive mode"),
            "expected permissive-mode warning, got: {captured}"
        );
        assert!(
            captured.contains("644"),
            "expected mode 644 in warning, got: {captured}"
        );

        // The pure detector agrees with the warning branch.
        assert_eq!(detect_loose_permissions(&db_path), Some(0o644));

        // The file MUST NOT have been forcibly fixed.
        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "file must not be auto-fixed, got {mode:o}");
    }
}

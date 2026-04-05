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

impl Database {
    /// Open (or create) the SQLite database at `path` and ensure schema exists.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

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
}

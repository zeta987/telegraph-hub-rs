use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use sha2::{Digest, Sha256};

use crate::db::Database;
use crate::error::AppError;
use crate::telegraph::client::TelegraphClient;

/// Time-to-live for cached page lists.
const CACHE_TTL_SECS: u64 = 300; // 5 minutes

/// Small courtesy delay between requests to avoid hammering the API.
const MIN_DELAY_MS: u64 = 50;

/// Maximum retries per batch request on FLOOD_WAIT or transient errors.
const MAX_RETRIES: u32 = 3;

/// Maximum items per Telegraph API `getPageList` call.
const FETCH_BATCH_SIZE: i32 = 200;

/// Lightweight summary of a Telegraph page (no content field).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageSummary {
    pub path: String,
    pub title: String,
    pub url: String,
    pub views: i64,
}

/// A cached list of page summaries with a creation timestamp.
#[derive(Debug, Clone)]
pub struct CachedPageList {
    pub pages: Vec<PageSummary>,
    pub total_count: i64,
    pub created_at: Instant,
}

impl CachedPageList {
    /// Check whether this cache entry has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= CACHE_TTL_SECS
    }
}

/// Progress tracker for a cache build operation.
/// Holds partial page data so search can show results while building.
#[derive(Debug)]
pub struct BuildProgress {
    pub fetched: AtomicUsize,
    pub total: AtomicUsize,
    pub complete: AtomicBool,
    pub error: std::sync::Mutex<Option<String>>,
    pub pages: std::sync::Mutex<Vec<PageSummary>>,
}

impl BuildProgress {
    fn new() -> Self {
        Self {
            fetched: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            complete: AtomicBool::new(false),
            error: std::sync::Mutex::new(None),
            pages: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl PageCache {
    /// Get a snapshot of partial pages from an in-progress build.
    /// Used by search to show results while cache is still building.
    pub fn get_partial_pages(&self, token_hash: &str) -> Option<Vec<PageSummary>> {
        let entry = self.progress.get(token_hash)?;
        Some(entry.pages.lock().unwrap().clone())
    }

    /// Mark specific pages as deleted in the cache without invalidating it.
    /// Updates title to `[DELETED]` in memory, in-progress builds, and SQLite.
    pub fn mark_deleted(&self, token_hash: &str, paths: &[String]) {
        let path_set: std::collections::HashSet<&str> = paths.iter().map(|p| p.as_str()).collect();

        // Update completed in-memory cache
        if let Some(mut entry) = self.inner.get_mut(token_hash) {
            for page in &mut entry.pages {
                if path_set.contains(page.path.as_str()) {
                    page.title = "[DELETED]".to_string();
                }
            }
        }

        // Update in-progress build's partial pages
        if let Some(progress) = self.progress.get(token_hash) {
            let mut pages = progress.pages.lock().unwrap();
            for page in pages.iter_mut() {
                if path_set.contains(page.path.as_str()) {
                    page.title = "[DELETED]".to_string();
                }
            }
        }

        // Update SQLite
        if let Some(db) = &self.db {
            let db = db.clone();
            let token_hash = token_hash.to_string();
            let paths = paths.to_vec();
            tokio::spawn(async move {
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    let db = db.lock().unwrap();
                    db.mark_deleted(&token_hash, &paths)
                })
                .await
                .unwrap_or_else(|e| Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))
                {
                    tracing::warn!("Failed to mark pages as deleted in database: {e}");
                }
            });
        }
    }
}

/// Per-token page metadata cache with optional SQLite persistence.
///
/// Keys are SHA-256 hashes of access tokens (raw tokens are never stored).
/// Values are `CachedPageList` with TTL-based expiration.
/// When a `Database` is attached, completed cache builds are persisted
/// and reloaded on startup so the cache survives process restarts.
#[derive(Clone)]
pub struct PageCache {
    inner: Arc<DashMap<String, CachedPageList>>,
    progress: Arc<DashMap<String, Arc<BuildProgress>>>,
    db: Option<Arc<std::sync::Mutex<Database>>>,
}

impl PageCache {
    /// Create a cache without persistence (for tests).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            progress: Arc::new(DashMap::new()),
            db: None,
        }
    }

    /// Create a cache backed by SQLite. Loads non-expired entries on startup.
    pub fn new_with_db(db: Database) -> Self {
        let inner = Arc::new(DashMap::new());

        // Load persisted cache entries that haven't expired
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs() as i64;

        match db.load_all() {
            Ok(entries) => {
                for entry in entries {
                    let token_hash = entry.token_hash;
                    let pages = entry.pages;
                    let total_count = entry.total_count;
                    let created_at_unix = entry.created_at_unix;
                    let age_secs = (now_unix - created_at_unix).max(0) as u64;
                    if age_secs < CACHE_TTL_SECS {
                        let created_at = Instant::now() - Duration::from_secs(age_secs);
                        inner.insert(
                            token_hash.clone(),
                            CachedPageList {
                                pages,
                                total_count,
                                created_at,
                            },
                        );
                        tracing::info!(
                            "Loaded cached page list for token {:.8}… ({} pages, {}s old)",
                            token_hash,
                            total_count,
                            age_secs,
                        );
                    } else {
                        tracing::debug!(
                            "Skipped expired cache for token {:.8}… ({}s old)",
                            token_hash,
                            age_secs,
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load cache from database: {e}");
            }
        }

        Self {
            inner,
            progress: Arc::new(DashMap::new()),
            db: Some(Arc::new(std::sync::Mutex::new(db))),
        }
    }

    /// Get a cached page list if it exists and has not expired.
    /// Returns `None` on cache miss or expiration.
    pub fn get(&self, token_hash: &str) -> Option<CachedPageList> {
        let entry = self.inner.get(token_hash)?;
        if entry.is_expired() {
            drop(entry);
            self.inner.remove(token_hash);
            None
        } else {
            Some(entry.clone())
        }
    }

    /// Check the progress of a cache build for a given token hash.
    /// Returns `(fetched, total, complete, error)`.
    pub fn get_progress(&self, token_hash: &str) -> Option<(usize, usize, bool, Option<String>)> {
        let entry = self.progress.get(token_hash)?;
        let fetched = entry.fetched.load(Ordering::Relaxed);
        let total = entry.total.load(Ordering::Relaxed);
        let complete = entry.complete.load(Ordering::Relaxed);
        let error = entry.error.lock().unwrap().clone();
        Some((fetched, total, complete, error))
    }

    /// Check if a build is already in progress for this token.
    pub fn is_building(&self, token_hash: &str) -> bool {
        self.progress
            .get(token_hash)
            .is_some_and(|p| !p.complete.load(Ordering::Relaxed))
    }

    /// Start a background cache build. Returns immediately.
    ///
    /// The build spawns a tokio task that fetches all pages from the
    /// Telegraph API with FLOOD_WAIT-aware rate limiting and tracks
    /// progress in `self.progress`.
    pub fn start_build(
        &self,
        token_hash: String,
        access_token: String,
        telegraph: TelegraphClient,
    ) {
        // Don't start a second build if one is already running
        if self.is_building(&token_hash) {
            return;
        }

        let progress = Arc::new(BuildProgress::new());
        self.progress.insert(token_hash.clone(), progress.clone());

        let inner = self.inner.clone();
        let progress_map = self.progress.clone();
        let db = self.db.clone();

        tokio::spawn(async move {
            match Self::do_build(&access_token, &telegraph, &progress).await {
                Ok(cached) => {
                    // Persist to SQLite in a blocking task
                    if let Some(db) = &db {
                        let db = db.clone();
                        let token_hash_clone = token_hash.clone();
                        let pages = cached.pages.clone();
                        let total_count = cached.total_count;
                        if let Err(e) = tokio::task::spawn_blocking(move || {
                            let mut db = db.lock().unwrap();
                            db.save(&token_hash_clone, &pages, total_count)
                        })
                        .await
                        .unwrap_or_else(|e| {
                            Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
                        }) {
                            tracing::warn!("Failed to persist cache to database: {e}");
                        }
                    }

                    inner.insert(token_hash.clone(), cached);
                    progress.complete.store(true, Ordering::Relaxed);
                }
                Err(e) => {
                    *progress.error.lock().unwrap() = Some(e.to_string());
                    progress.complete.store(true, Ordering::Relaxed);
                }
            }
            // Clean up progress entry after a short delay so the final poll can read it
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            progress_map.remove(&token_hash);
        });
    }

    /// Internal: fetch all pages from Telegraph API with FLOOD_WAIT handling.
    async fn do_build(
        access_token: &str,
        telegraph: &TelegraphClient,
        progress: &BuildProgress,
    ) -> Result<CachedPageList, AppError> {
        let mut all_pages: Vec<PageSummary> = Vec::new();
        let mut offset = 0i32;

        // First request to discover total_count
        let first_batch = telegraph
            .get_page_list(access_token, Some(offset), Some(FETCH_BATCH_SIZE))
            .await?;
        let mut total_count = first_batch.total_count;
        progress
            .total
            .store(total_count as usize, Ordering::Relaxed);

        for page in &first_batch.pages {
            let summary = PageSummary {
                path: page.path.clone(),
                title: page.title.clone(),
                url: page.url.clone(),
                views: page.views,
            };
            all_pages.push(summary.clone());
            progress.pages.lock().unwrap().push(summary);
        }
        offset += first_batch.pages.len() as i32;
        progress.fetched.store(offset as usize, Ordering::Relaxed);

        // Fetch remaining pages with FLOOD_WAIT-aware retry
        while (offset as i64) < total_count {
            // Small courtesy delay to avoid hammering
            tokio::time::sleep(std::time::Duration::from_millis(MIN_DELAY_MS)).await;

            let mut last_err = None;
            let mut success = false;

            for attempt in 0..MAX_RETRIES {
                match telegraph
                    .get_page_list(access_token, Some(offset), Some(FETCH_BATCH_SIZE))
                    .await
                {
                    Ok(batch) => {
                        total_count = batch.total_count;
                        progress
                            .total
                            .store(total_count as usize, Ordering::Relaxed);

                        {
                            let mut shared = progress.pages.lock().unwrap();
                            for page in &batch.pages {
                                let summary = PageSummary {
                                    path: page.path.clone(),
                                    title: page.title.clone(),
                                    url: page.url.clone(),
                                    views: page.views,
                                };
                                all_pages.push(summary.clone());
                                shared.push(summary);
                            }
                        }
                        offset += batch.pages.len() as i32;
                        progress.fetched.store(offset as usize, Ordering::Relaxed);

                        if batch.pages.is_empty() {
                            total_count = offset as i64;
                        }
                        success = true;
                        break;
                    }
                    Err(e) => {
                        // Check for FLOOD_WAIT_X pattern
                        if let Some(wait_secs) = parse_flood_wait(&e) {
                            tracing::warn!(
                                "Cache build: FLOOD_WAIT_{wait_secs} at offset {offset}, waiting..."
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                            // Don't count as an attempt — retry immediately after wait
                            continue;
                        }

                        last_err = Some(e);
                        // Generic error: exponential backoff 2s, 4s, 8s
                        let backoff = 2000 * (1 << attempt);
                        tracing::warn!(
                            "Cache build: request at offset {offset} failed (attempt {}), retrying in {backoff}ms",
                            attempt + 1
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                    }
                }
            }
            if !success {
                return Err(last_err.unwrap());
            }
        }

        Ok(CachedPageList {
            total_count,
            pages: all_pages,
            created_at: Instant::now(),
        })
    }

    /// Remove the cached entry for a given token hash (from memory and SQLite).
    pub fn invalidate(&self, token_hash: &str) {
        self.inner.remove(token_hash);

        if let Some(db) = &self.db {
            let db = db.clone();
            let token_hash = token_hash.to_string();
            // Fire-and-forget: invalidation failure is non-critical
            tokio::spawn(async move {
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    let db = db.lock().unwrap();
                    db.invalidate(&token_hash)
                })
                .await
                .unwrap_or_else(|e| Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))
                {
                    tracing::warn!("Failed to invalidate cache in database: {e}");
                }
            });
        }
    }
}

/// Check if an error is a Telegraph FLOOD_WAIT, and extract the wait duration in seconds.
fn parse_flood_wait(err: &AppError) -> Option<u64> {
    if let AppError::Telegraph(msg) = err {
        msg.strip_prefix("FLOOD_WAIT_")?.parse().ok()
    } else {
        None
    }
}

/// Compute the SHA-256 hash of an access token, returned as a hex string.
pub fn hash_token(access_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(access_token.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_deterministic() {
        let h1 = hash_token("abc123");
        let h2 = hash_token("abc123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn hash_token_different_inputs() {
        let h1 = hash_token("token_a");
        let h2 = hash_token("token_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn cache_miss_on_empty() {
        let cache = PageCache::new();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn cache_invalidate_removes_entry() {
        let cache = PageCache::new();
        cache.inner.insert(
            "test_hash".to_string(),
            CachedPageList {
                pages: vec![],
                total_count: 0,
                created_at: Instant::now(),
            },
        );
        assert!(cache.get("test_hash").is_some());
        cache.invalidate("test_hash");
        assert!(cache.get("test_hash").is_none());
    }

    #[test]
    fn cache_expired_entry_returns_none() {
        let cache = PageCache::new();
        cache.inner.insert(
            "old_hash".to_string(),
            CachedPageList {
                pages: vec![],
                total_count: 0,
                created_at: Instant::now() - std::time::Duration::from_secs(CACHE_TTL_SECS + 1),
            },
        );
        assert!(cache.get("old_hash").is_none());
    }

    #[test]
    fn cache_fresh_entry_returns_some() {
        let cache = PageCache::new();
        let entry = CachedPageList {
            pages: vec![PageSummary {
                path: "test-page".to_string(),
                title: "Test".to_string(),
                url: "https://telegra.ph/test-page".to_string(),
                views: 42,
            }],
            total_count: 1,
            created_at: Instant::now(),
        };
        cache.inner.insert("fresh_hash".to_string(), entry);
        let result = cache.get("fresh_hash").unwrap();
        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].path, "test-page");
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn parse_flood_wait_valid() {
        let err = AppError::Telegraph("FLOOD_WAIT_5".to_string());
        assert_eq!(parse_flood_wait(&err), Some(5));
    }

    #[test]
    fn parse_flood_wait_invalid() {
        let err = AppError::Telegraph("INVALID_TOKEN".to_string());
        assert_eq!(parse_flood_wait(&err), None);
    }

    #[test]
    fn parse_flood_wait_non_telegraph() {
        let err = AppError::Template(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "test",
        ));
        assert_eq!(parse_flood_wait(&err), None);
    }
}

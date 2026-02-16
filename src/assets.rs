use std::time::Duration;

use ic_asset_certification::{Asset, AssetConfig, AssetEncoding};
use ic_cdk::api::certified_data_set;
use ic_http_certification::HeaderField;
use include_dir::Dir;

use crate::{mime::get_mime_type, ASSET_ROUTER, DYNAMIC_CACHE, ROUTER_CONFIG};

/// Metadata for a dynamically generated asset cached by the library.
///
/// Tracks when the asset was certified and its optional TTL, enabling
/// TTL-based cache invalidation in the query path.
pub struct CachedDynamicAsset {
    /// Timestamp (nanoseconds since UNIX epoch) when the asset was certified.
    /// Obtained via `ic_cdk::api::time()`.
    pub certified_at: u64,
    /// Optional time-to-live. If `None`, the asset is cached indefinitely
    /// (backwards-compatible with pre-TTL behavior).
    pub ttl: Option<Duration>,
}

impl CachedDynamicAsset {
    /// Returns `true` if this asset has expired based on the given current time
    /// (nanoseconds since UNIX epoch).
    ///
    /// An asset without a TTL never expires.
    pub fn is_expired(&self, now_ns: u64) -> bool {
        match self.ttl {
            None => false,
            Some(ttl) => {
                let expiry_ns = self.certified_at.saturating_add(ttl.as_nanos() as u64);
                now_ns >= expiry_ns
            }
        }
    }
}

// Cache-control values are now configurable via `CacheControl` in `src/config.rs`.
pub fn certify_all_assets(asset_dir: &Dir<'static>) {
    let encodings = vec![
        AssetEncoding::Brotli.default_config(),
        AssetEncoding::Gzip.default_config(),
    ];

    let mut assets: Vec<Asset<'static, 'static>> = Vec::new();
    let mut asset_configs: Vec<AssetConfig> = Vec::new();

    collect_assets_with_config(asset_dir, &mut assets, &mut asset_configs, encodings);

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        if let Err(err) = asset_router.certify_assets(assets, asset_configs) {
            ic_cdk::trap(format!("Failed to certify assets: {err}"));
        }
    });

    // Set certified data AFTER all tree modifications
    ASSET_ROUTER.with_borrow(|asset_router| {
        certified_data_set(asset_router.root_hash());
    });
}

/// Recursively collects all files from the given directory into a flat list of [`Asset`] values.
/// Unlike [`certify_all_assets`], this does not configure MIME types, headers, or encodings —
/// it is useful when consumers need raw asset data for custom processing or certification logic.
pub fn collect_assets(dir: &Dir<'_>, assets: &mut Vec<Asset<'static, 'static>>) {
    for file in dir.files() {
        let raw_path = file.path().to_string_lossy().to_string();
        let path = if raw_path.starts_with('/') {
            raw_path
        } else {
            format!("/{raw_path}")
        };
        assets.push(Asset::new(path, file.contents().to_vec()));
    }

    for subdir in dir.dirs() {
        collect_assets(subdir, assets);
    }
}

fn collect_assets_with_config(
    dir: &Dir<'_>,
    assets: &mut Vec<Asset<'static, 'static>>,
    asset_configs: &mut Vec<AssetConfig>,
    encodings: Vec<(AssetEncoding, String)>,
) {
    for file in dir.files() {
        let raw_path = file.path().to_string_lossy().to_string();
        // include_dir stores relative paths (e.g. "style.css") but HTTP
        // requests use absolute paths ("/style.css"). Ensure a leading slash.
        let path = if raw_path.starts_with('/') {
            raw_path
        } else {
            format!("/{raw_path}")
        };

        assets.push(Asset::new(path.clone(), file.contents().to_vec()));

        let mime_type = get_mime_type(&path);
        let use_encodings = if mime_type.starts_with("text/")
            || mime_type == "application/javascript"
            || mime_type == "application/json"
            || mime_type == "application/xml"
            || mime_type == "image/svg+xml"
        {
            encodings.clone()
        } else {
            vec![]
        };

        let static_cache_control =
            ROUTER_CONFIG.with(|c| c.borrow().cache_control.static_assets.clone());
        asset_configs.push(AssetConfig::File {
            path,
            content_type: Some(mime_type.to_string()),
            headers: get_asset_headers(vec![("cache-control".to_string(), static_cache_control)]),
            fallback_for: vec![],
            aliased_by: vec![],
            encodings: use_encodings,
        });
    }

    for subdir in dir.dirs() {
        collect_assets_with_config(subdir, assets, asset_configs, encodings.clone());
    }
}

/// Build the header list for an asset by merging the global [`ROUTER_CONFIG`]
/// security headers and custom headers with any per-call `additional_headers`.
///
/// Merge order (last-write-wins for duplicate header names):
/// 1. Security headers (from global config)
/// 2. Custom headers (from global config)
/// 3. `additional_headers` (per-route / per-call overrides)
pub fn get_asset_headers(additional_headers: Vec<HeaderField>) -> Vec<HeaderField> {
    ROUTER_CONFIG.with(|c| c.borrow().merged_headers(additional_headers))
}

pub fn delete_assets(asset_paths: Vec<&str>) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        asset_router.delete_assets_by_path(asset_paths);
        certified_data_set(asset_router.root_hash());
    });
}

/// Invalidate a single cached dynamic asset by exact path.
///
/// Removes the path from the asset router and from the dynamic cache,
/// then updates the root hash. The next request to this path will trigger an
/// update call to regenerate the asset.
///
/// Static assets (not in `DYNAMIC_CACHE`) are unaffected.
pub fn invalidate_path(path: &str) {
    let was_dynamic = DYNAMIC_CACHE.with(|dc| dc.borrow_mut().remove(path).is_some());
    if was_dynamic {
        ASSET_ROUTER.with_borrow_mut(|asset_router| {
            asset_router.delete_assets_by_path(vec![path]);
            certified_data_set(asset_router.root_hash());
        });
    }
}

/// Invalidate all cached dynamic assets whose path starts with the given prefix.
///
/// Example: `invalidate_prefix("/posts/")` clears `/posts/1`, `/posts/2`, etc.
///
/// Static assets are unaffected.
pub fn invalidate_prefix(prefix: &str) {
    let to_remove: Vec<String> = DYNAMIC_CACHE.with(|dc| {
        dc.borrow()
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect()
    });

    if to_remove.is_empty() {
        return;
    }

    DYNAMIC_CACHE.with(|dc| {
        let mut map = dc.borrow_mut();
        for p in &to_remove {
            map.remove(p);
        }
    });

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let refs: Vec<&str> = to_remove.iter().map(|s| s.as_str()).collect();
        asset_router.delete_assets_by_path(refs);
        certified_data_set(asset_router.root_hash());
    });
}

/// Invalidate all dynamically generated assets.
///
/// Static assets (embedded at compile time) are unaffected.
pub fn invalidate_all_dynamic() {
    let all: Vec<String> = DYNAMIC_CACHE.with(|dc| {
        let mut map = dc.borrow_mut();
        let paths: Vec<String> = map.drain().map(|(k, _)| k).collect();
        paths
    });

    if all.is_empty() {
        return;
    }

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let refs: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
        asset_router.delete_assets_by_path(refs);
        certified_data_set(asset_router.root_hash());
    });
}

/// Returns `true` if the given path is registered as a dynamic asset.
///
/// This is primarily useful for testing and debugging.
pub fn is_dynamic_path(path: &str) -> bool {
    DYNAMIC_CACHE.with(|dc| dc.borrow().contains_key(path))
}

/// Returns the number of registered dynamic asset paths.
///
/// This is primarily useful for testing and debugging.
pub fn dynamic_path_count() -> usize {
    DYNAMIC_CACHE.with(|dc| dc.borrow().len())
}

/// Register a path as a dynamic asset in the internal cache.
///
/// This is a low-level operation exposed for testing; normal usage should rely
/// on `http_request_update` to register dynamic paths automatically.
///
/// Inserts a `CachedDynamicAsset` with `certified_at: 0` and `ttl: None`.
pub fn register_dynamic_path(path: &str) {
    DYNAMIC_CACHE.with(|dc| {
        dc.borrow_mut().insert(
            path.to_string(),
            CachedDynamicAsset {
                certified_at: 0,
                ttl: None,
            },
        );
    });
}

/// Remove a path from the dynamic asset cache *without* touching the
/// asset router or certification tree.
///
/// This is the cache-only counterpart of [`invalidate_path`] and exists
/// to allow unit tests that cannot call IC runtime APIs.
#[cfg(test)]
fn remove_dynamic_path(path: &str) -> bool {
    DYNAMIC_CACHE.with(|dc| dc.borrow_mut().remove(path).is_some())
}

/// Remove all paths matching a prefix from the dynamic asset cache
/// *without* touching the asset router or certification tree.
#[cfg(test)]
fn remove_dynamic_prefix(prefix: &str) -> Vec<String> {
    let to_remove: Vec<String> = DYNAMIC_CACHE.with(|dc| {
        dc.borrow()
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect()
    });
    DYNAMIC_CACHE.with(|dc| {
        let mut map = dc.borrow_mut();
        for p in &to_remove {
            map.remove(p);
        }
    });
    to_remove
}

/// Clear all entries from the dynamic asset cache *without* touching the
/// asset router or certification tree.
#[cfg(test)]
fn clear_dynamic_paths() -> Vec<String> {
    DYNAMIC_CACHE.with(|dc| {
        let mut map = dc.borrow_mut();
        map.drain().map(|(k, _)| k).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: reset DYNAMIC_CACHE before each test to avoid cross-test leakage
    /// (thread-local state persists across tests in the same thread).
    fn reset_dynamic_paths() {
        DYNAMIC_CACHE.with(|dc| dc.borrow_mut().clear());
    }

    // ---- 4.2.9: invalidate_path removes the path from DYNAMIC_PATHS ----

    #[test]
    fn invalidate_path_removes_from_dynamic_paths() {
        reset_dynamic_paths();
        register_dynamic_path("/posts/42");
        assert!(is_dynamic_path("/posts/42"));

        // Use the registry-only removal (mirrors invalidate_path logic without
        // calling certified_data_set which is unavailable in unit tests).
        let removed = remove_dynamic_path("/posts/42");
        assert!(removed);
        assert!(!is_dynamic_path("/posts/42"));
    }

    // ---- 4.2.10: invalidate_prefix removes matching, keeps non-matching ----

    #[test]
    fn invalidate_prefix_removes_matching_keeps_others() {
        reset_dynamic_paths();
        register_dynamic_path("/posts/1");
        register_dynamic_path("/posts/2");
        register_dynamic_path("/about");

        let removed = remove_dynamic_prefix("/posts/");
        assert_eq!(removed.len(), 2);
        assert!(!is_dynamic_path("/posts/1"));
        assert!(!is_dynamic_path("/posts/2"));
        assert!(is_dynamic_path("/about"));
    }

    // ---- 4.2.11: invalidate_all_dynamic clears all entries ----

    #[test]
    fn invalidate_all_dynamic_clears_all() {
        reset_dynamic_paths();
        register_dynamic_path("/posts/1");
        register_dynamic_path("/posts/2");
        register_dynamic_path("/about");
        assert_eq!(dynamic_path_count(), 3);

        let removed = clear_dynamic_paths();
        assert_eq!(removed.len(), 3);
        assert_eq!(dynamic_path_count(), 0);
    }

    // ---- 4.2.12: static assets unaffected by invalidation ----

    #[test]
    fn static_assets_unaffected_by_invalidation() {
        reset_dynamic_paths();
        // Only "/posts/1" is dynamic; "/style.css" is a static asset (never
        // registered in DYNAMIC_PATHS).
        register_dynamic_path("/posts/1");

        // invalidate_path on a non-dynamic path is a no-op
        let removed = remove_dynamic_path("/style.css");
        assert!(!removed);

        // invalidate_prefix on a prefix that only matches static paths is a no-op
        let removed = remove_dynamic_prefix("/style");
        assert!(removed.is_empty());

        // The dynamic path is still there
        assert!(is_dynamic_path("/posts/1"));

        // Also test that invalidate_path with a non-dynamic path doesn't panic
        // (the real function guards on DYNAMIC_CACHE membership)
        invalidate_path("/style.css");
        // No panic, and /posts/1 still registered
        assert!(is_dynamic_path("/posts/1"));
    }

    // ---- 4.1.17: CachedDynamicAsset with ttl: None never expires ----

    #[test]
    fn cached_dynamic_asset_no_ttl_never_expires() {
        let asset = CachedDynamicAsset {
            certified_at: 1_000_000_000_000_000_000, // 1 second in nanoseconds
            ttl: None,
        };
        // Even far in the future, is_expired returns false
        assert!(!asset.is_expired(u64::MAX));
        assert!(!asset.is_expired(0));
        assert!(!asset.is_expired(asset.certified_at));
    }

    // ---- 4.1.18: CachedDynamicAsset with expired TTL is detected ----

    #[test]
    fn cached_dynamic_asset_expired_ttl_detected() {
        let one_hour_ns: u64 = 3_600_000_000_000; // 3600 seconds in nanoseconds
        let asset = CachedDynamicAsset {
            certified_at: 1_000_000_000_000_000_000, // some past time
            ttl: Some(Duration::from_secs(3600)),
        };
        // Time well past the expiry
        let now_expired = asset.certified_at + one_hour_ns + 1;
        assert!(asset.is_expired(now_expired));

        // Exactly at expiry boundary
        let now_at_boundary = asset.certified_at + one_hour_ns;
        assert!(asset.is_expired(now_at_boundary));
    }

    // ---- 4.1.19: CachedDynamicAsset with fresh TTL is not expired ----

    #[test]
    fn cached_dynamic_asset_fresh_ttl_not_expired() {
        let one_hour_ns: u64 = 3_600_000_000_000;
        let asset = CachedDynamicAsset {
            certified_at: 1_000_000_000_000_000_000,
            ttl: Some(Duration::from_secs(3600)),
        };
        // Still within TTL window
        let now_fresh = asset.certified_at + one_hour_ns - 1;
        assert!(!asset.is_expired(now_fresh));

        // Immediately after certification
        assert!(!asset.is_expired(asset.certified_at));
        assert!(!asset.is_expired(asset.certified_at + 1));
    }
}

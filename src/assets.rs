use ic_cdk::api::certified_data_set;
use ic_http_certification::HeaderField;
use include_dir::Dir;

use crate::asset_router::{AssetCertificationConfig, AssetEncoding, AssetRouter};
use crate::certification::CertificationMode;
use crate::{mime::get_mime_type, ASSET_ROUTER, ROUTER_CONFIG};

/// Certify all static assets from the given embedded directory using the
/// default certification mode ([`CertificationMode::ResponseOnly`]).
///
/// This is the simple, common-case API. For explicit control over the
/// certification mode, use [`certify_assets_with_mode`].
///
/// Call this during canister initialization (e.g. in `init` and
/// `post_upgrade`) after calling [`set_asset_config`](crate::set_asset_config).
///
/// # Example
///
/// ```rust,ignore
/// certify_assets(&include_dir!("assets"));
/// ```
pub fn certify_assets(asset_dir: &Dir<'static>) {
    certify_assets_with_mode(asset_dir, CertificationMode::response_only())
}

/// Certify all assets in the given directory with the specified certification mode.
///
/// Walks `asset_dir` recursively, determines MIME types, applies the global
/// [`CacheControl`](crate::CacheControl) and [`SecurityHeaders`](crate::SecurityHeaders)
/// configuration, and registers each file with the certification tree using
/// the provided [`CertificationMode`].
///
/// Call this multiple times with different directories and modes to set up
/// asset certification with varying security levels.
///
/// # Example
///
/// ```rust,ignore
/// // Static assets: response-only certification (default)
/// certify_assets(&include_dir!("assets/static"));
///
/// // Public files: skip certification entirely
/// certify_assets_with_mode(
///     &include_dir!("assets/public"),
///     CertificationMode::skip()
/// );
/// ```
pub fn certify_assets_with_mode(asset_dir: &Dir<'static>, mode: CertificationMode) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        certify_dir_recursive(asset_router, asset_dir, &mode);
    });

    // Set certified data AFTER all tree modifications
    ASSET_ROUTER.with_borrow(|asset_router| {
        certified_data_set(&asset_router.root_hash());
    });
}

/// Recursively certify all files in the directory and its subdirectories.
fn certify_dir_recursive(router: &mut AssetRouter, dir: &Dir<'static>, mode: &CertificationMode) {
    for file in dir.files() {
        let raw_path = file.path().to_string_lossy().to_string();

        // Skip pre-compressed variants — they're collected by
        // `collect_encoded_variants` and attached to the original asset.
        if raw_path.ends_with(".br") || raw_path.ends_with(".gz") {
            continue;
        }

        let path = if raw_path.starts_with('/') {
            raw_path
        } else {
            format!("/{raw_path}")
        };
        let content = file.contents().to_vec();

        let mime_type = get_mime_type(&path);

        // Collect pre-compressed encodings (.br, .gz) from the directory.
        // Only text-like assets are expected to have compressed variants.
        let use_encodings = if mime_type.starts_with("text/")
            || mime_type == "application/javascript"
            || mime_type == "application/json"
            || mime_type == "application/xml"
            || mime_type == "image/svg+xml"
        {
            let rel_path = file.path().to_string_lossy().to_string();
            collect_encoded_variants(dir, &rel_path)
        } else {
            vec![]
        };

        let static_cache_control =
            ROUTER_CONFIG.with(|c| c.borrow().cache_control.static_assets.clone());

        let config = AssetCertificationConfig {
            mode: mode.clone(),
            content_type: Some(mime_type.to_string()),
            headers: get_asset_headers(vec![("cache-control".to_string(), static_cache_control)]),
            encodings: use_encodings,
            certified_at: 0,
            ttl: None, // Static assets don't expire
            ..Default::default()
        };

        // Auto-generate aliases: index.html → directory paths.
        let config = if path.ends_with("/index.html") {
            let dir_path = path.trim_end_matches("index.html").to_string();
            let dir_path_no_trailing = dir_path.trim_end_matches('/').to_string();
            let mut aliases = vec![dir_path];
            if !dir_path_no_trailing.is_empty() {
                aliases.push(dir_path_no_trailing);
            }
            AssetCertificationConfig { aliases, ..config }
        } else {
            config
        };

        if let Err(err) = router.certify_asset(&path, content, config) {
            ic_cdk::trap(format!("Failed to certify asset {path}: {err}"));
        }
    }

    // Recurse into subdirectories
    for subdir in dir.dirs() {
        certify_dir_recursive(router, subdir, mode);
    }
}

/// Collect pre-compressed encoding variants for a file from the directory.
///
/// Looks for sibling files with `.br` (Brotli) and `.gz` (Gzip) extensions.
/// For example, given `style.css`, this looks for `style.css.br` and
/// `style.css.gz` in the same directory.
fn collect_encoded_variants(dir: &Dir<'static>, file_path: &str) -> Vec<(AssetEncoding, Vec<u8>)> {
    let mut encodings = Vec::new();

    let br_path = format!("{}.br", file_path);
    let gz_path = format!("{}.gz", file_path);

    for file in dir.files() {
        let p = file.path().to_string_lossy().to_string();
        if p == br_path {
            encodings.push((AssetEncoding::Brotli, file.contents().to_vec()));
        } else if p == gz_path {
            encodings.push((AssetEncoding::Gzip, file.contents().to_vec()));
        }
    }

    encodings
}

/// Build the header list for an asset by merging the global router configuration's
/// security headers and custom headers with any per-call `additional_headers`.
///
/// Merge order (last-write-wins for duplicate header names):
/// 1. Security headers (from global config)
/// 2. Custom headers (from global config)
/// 3. `additional_headers` (per-route / per-call overrides)
pub fn get_asset_headers(additional_headers: Vec<HeaderField>) -> Vec<HeaderField> {
    ROUTER_CONFIG.with(|c| c.borrow().merged_headers(additional_headers))
}

/// Delete previously certified assets by their paths.
///
/// Removes the assets from the certification tree and updates the root hash.
/// This is a low-level operation; prefer [`invalidate_path`] or
/// [`invalidate_prefix`] for dynamic asset invalidation.
pub fn delete_assets(asset_paths: Vec<&str>) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        for path in asset_paths {
            asset_router.delete_asset(path);
        }
        certified_data_set(&asset_router.root_hash());
    });
}

/// Invalidate a single cached dynamic asset by exact path.
///
/// Removes the path from the asset router,
/// then updates the root hash. The next request to this path will trigger an
/// update call to regenerate the asset.
///
/// Static assets (those without a TTL) are unaffected.
///
/// # Examples
///
/// ```rust,ignore
/// use ic_asset_router::invalidate_path;
///
/// // After updating a blog post, force regeneration on next request:
/// invalidate_path("/posts/42");
/// ```
pub fn invalidate_path(path: &str) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let is_dynamic = asset_router
            .get_asset(path)
            .map(|a| a.is_dynamic())
            .unwrap_or(false);
        if is_dynamic {
            asset_router.delete_asset(path);
            certified_data_set(&asset_router.root_hash());
        }
    });
}

/// Invalidate all cached dynamic assets whose path starts with the given prefix.
///
/// Static assets are unaffected.
///
/// # Examples
///
/// ```rust,ignore
/// use ic_asset_router::invalidate_prefix;
///
/// // Clear all cached posts after a bulk update:
/// invalidate_prefix("/posts/");
/// // Clears /posts/1, /posts/2, etc. but not /postscript
/// ```
pub fn invalidate_prefix(prefix: &str) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let to_remove = asset_router.dynamic_paths_with_prefix(prefix);
        if to_remove.is_empty() {
            return;
        }
        for p in &to_remove {
            asset_router.delete_asset(p);
        }
        certified_data_set(&asset_router.root_hash());
    });
}

/// Invalidate all dynamically generated assets.
///
/// Static assets (embedded at compile time) are unaffected.
///
/// # Examples
///
/// ```rust,ignore
/// use ic_asset_router::invalidate_all_dynamic;
///
/// // Nuclear option: force regeneration of every dynamic page:
/// invalidate_all_dynamic();
/// ```
pub fn invalidate_all_dynamic() {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let all = asset_router.dynamic_paths();
        if all.is_empty() {
            return;
        }
        for p in &all {
            asset_router.delete_asset(p);
        }
        certified_data_set(&asset_router.root_hash());
    });
}

/// Returns the certification timestamp for an asset, if it exists.
///
/// Handlers can use this to decide whether regeneration is actually needed:
/// if the underlying data hasn't changed since `last_certified_at`, the handler
/// can return [`HandlerResult::NotModified`](crate::HandlerResult::NotModified)
/// to skip recertification.
///
/// Returns `None` if the path is not in the asset router (i.e., it has never
/// been generated, or has been invalidated).
///
/// # Examples
///
/// ```rust,ignore
/// use ic_asset_router::{last_certified_at, HandlerResult};
///
/// fn result_handler(req: HttpRequest, params: RouteParams) -> HandlerResult {
///     if let Some(ts) = last_certified_at("/posts/42") {
///         let data_updated_at = get_last_update_timestamp(); // your logic
///         if data_updated_at <= ts {
///             return HandlerResult::NotModified;
///         }
///     }
///     // Data changed — regenerate
///     HandlerResult::Response(build_response())
/// }
/// ```
pub fn last_certified_at(path: &str) -> Option<u64> {
    ASSET_ROUTER
        .with_borrow(|asset_router| asset_router.get_asset(path).map(|asset| asset.certified_at))
}

/// Returns `true` if the given path is registered as a dynamic asset.
///
/// This is primarily useful for testing and debugging.
pub fn is_dynamic_path(path: &str) -> bool {
    ASSET_ROUTER.with_borrow(|asset_router| {
        asset_router
            .get_asset(path)
            .map(|a| a.is_dynamic())
            .unwrap_or(false)
    })
}

/// Returns the number of registered dynamic asset paths.
///
/// This is primarily useful for testing and debugging.
pub fn dynamic_path_count() -> usize {
    ASSET_ROUTER.with_borrow(|asset_router| asset_router.dynamic_paths().len())
}

/// Register a path as a dynamic asset in the router.
///
/// This is a low-level operation exposed for testing; normal usage should rely
/// on `http_request_update` to register dynamic paths automatically.
///
/// Creates a minimal dynamic asset with `certified_at: 0` and the given
/// TTL. The asset body is empty.
pub fn register_dynamic_path(path: &str) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        let config = AssetCertificationConfig {
            mode: CertificationMode::skip(),
            certified_at: 0,
            ttl: Some(std::time::Duration::from_secs(3600)),
            dynamic: true,
            ..Default::default()
        };
        // Ignore errors — this is for testing only.
        let _ = asset_router.certify_asset(path, vec![], config);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper: reset the ASSET_ROUTER before each test to avoid cross-test leakage.
    fn reset_router() {
        ASSET_ROUTER.with_borrow_mut(|router| {
            // Delete all assets by collecting all canonical paths first.
            let all_paths: Vec<String> = router.dynamic_paths();
            for p in all_paths {
                router.delete_asset(&p);
            }
        });
    }

    // Helper: register a dynamic asset with given certified_at and ttl.
    fn register_dynamic_with_ttl(path: &str, certified_at: u64, ttl: Option<Duration>) {
        ASSET_ROUTER.with_borrow_mut(|asset_router| {
            let config = AssetCertificationConfig {
                mode: CertificationMode::skip(),
                certified_at,
                ttl,
                dynamic: true,
                ..Default::default()
            };
            let _ = asset_router.certify_asset(path, vec![], config);
        });
    }

    // ---- 4.2.9: invalidate_path removes the path from dynamic assets ----
    //
    // Note: invalidate_path/invalidate_prefix/invalidate_all_dynamic call
    // `certified_data_set()` which is unavailable in unit tests. We test the
    // underlying router logic directly instead.

    #[test]
    fn invalidate_path_removes_dynamic_asset() {
        reset_router();
        register_dynamic_path("/posts/42");
        assert!(is_dynamic_path("/posts/42"));

        // Directly test the invalidation logic (without certified_data_set).
        ASSET_ROUTER.with_borrow_mut(|router| {
            let is_dynamic = router
                .get_asset("/posts/42")
                .map(|a| a.is_dynamic())
                .unwrap_or(false);
            assert!(is_dynamic);
            router.delete_asset("/posts/42");
        });
        assert!(!is_dynamic_path("/posts/42"));
    }

    // ---- 4.2.10: invalidate_prefix removes matching, keeps non-matching ----

    #[test]
    fn invalidate_prefix_removes_matching_keeps_others() {
        reset_router();
        register_dynamic_path("/posts/1");
        register_dynamic_path("/posts/2");
        register_dynamic_path("/about");

        ASSET_ROUTER.with_borrow_mut(|router| {
            let to_remove = router.dynamic_paths_with_prefix("/posts/");
            assert_eq!(to_remove.len(), 2);
            for p in &to_remove {
                router.delete_asset(p);
            }
        });
        assert!(!is_dynamic_path("/posts/1"));
        assert!(!is_dynamic_path("/posts/2"));
        assert!(is_dynamic_path("/about"));
    }

    // ---- 4.2.11: invalidate_all_dynamic clears all entries ----

    #[test]
    fn invalidate_all_dynamic_clears_all() {
        reset_router();
        register_dynamic_path("/posts/1");
        register_dynamic_path("/posts/2");
        register_dynamic_path("/about");
        assert_eq!(dynamic_path_count(), 3);

        ASSET_ROUTER.with_borrow_mut(|router| {
            let all = router.dynamic_paths();
            for p in &all {
                router.delete_asset(p);
            }
        });
        assert_eq!(dynamic_path_count(), 0);
    }

    // ---- 4.2.12: static assets unaffected by invalidation ----

    #[test]
    fn static_assets_unaffected_by_invalidation() {
        reset_router();
        // Register a dynamic path
        register_dynamic_path("/posts/1");

        // Register a static path (no TTL → not dynamic)
        ASSET_ROUTER.with_borrow_mut(|asset_router| {
            let config = AssetCertificationConfig {
                mode: CertificationMode::skip(),
                certified_at: 0,
                ttl: None,
                ..Default::default()
            };
            let _ = asset_router.certify_asset("/style.css", b"body{}".to_vec(), config);
        });

        // Invalidating a static asset (via dynamic check) is a no-op.
        ASSET_ROUTER.with_borrow_mut(|router| {
            let is_dynamic = router
                .get_asset("/style.css")
                .map(|a| a.is_dynamic())
                .unwrap_or(false);
            assert!(!is_dynamic);
            // Don't delete — mirrors invalidate_path behavior for non-dynamic.
        });
        assert!(ASSET_ROUTER.with_borrow(|r| r.contains_asset("/style.css")));

        // The dynamic path is still there
        assert!(is_dynamic_path("/posts/1"));
    }

    // ---- CertifiedAsset TTL tests (previously CachedDynamicAsset tests) ----

    #[test]
    fn certified_asset_no_ttl_never_expires() {
        reset_router();
        ASSET_ROUTER.with_borrow_mut(|asset_router| {
            let config = AssetCertificationConfig {
                mode: CertificationMode::skip(),
                certified_at: 1_000_000_000_000_000_000,
                ttl: None,
                ..Default::default()
            };
            let _ = asset_router.certify_asset("/page", b"content".to_vec(), config);
        });
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            assert!(!asset.is_expired(u64::MAX));
            assert!(!asset.is_expired(0));
            assert!(!asset.is_expired(asset.certified_at));
        });
    }

    #[test]
    fn certified_asset_expired_ttl_detected() {
        reset_router();
        let one_hour_ns: u64 = 3_600_000_000_000;
        register_dynamic_with_ttl(
            "/page",
            1_000_000_000_000_000_000,
            Some(Duration::from_secs(3600)),
        );
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            let now_expired = asset.certified_at + one_hour_ns + 1;
            assert!(asset.is_expired(now_expired));
            let now_at_boundary = asset.certified_at + one_hour_ns;
            assert!(asset.is_expired(now_at_boundary));
        });
    }

    #[test]
    fn certified_asset_fresh_ttl_not_expired() {
        reset_router();
        let one_hour_ns: u64 = 3_600_000_000_000;
        register_dynamic_with_ttl(
            "/page",
            1_000_000_000_000_000_000,
            Some(Duration::from_secs(3600)),
        );
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            let now_fresh = asset.certified_at + one_hour_ns - 1;
            assert!(!asset.is_expired(now_fresh));
            assert!(!asset.is_expired(asset.certified_at));
            assert!(!asset.is_expired(asset.certified_at + 1));
        });
    }

    // ---- last_certified_at tests ----

    #[test]
    fn last_certified_at_returns_none_for_uncached() {
        reset_router();
        assert_eq!(last_certified_at("/nonexistent"), None);
    }

    #[test]
    fn last_certified_at_returns_some_for_cached() {
        reset_router();
        let timestamp = 1_000_000_000_000_000_000u64;
        register_dynamic_with_ttl("/posts/1", timestamp, Some(Duration::from_secs(3600)));
        assert_eq!(last_certified_at("/posts/1"), Some(timestamp));
        assert_eq!(last_certified_at("/posts/2"), None);
    }

    // ---- NotModified tests ----

    #[test]
    fn not_modified_preserves_asset_entry() {
        reset_router();
        let original_time = 1_000_000_000_000_000_000u64;
        register_dynamic_with_ttl("/posts/1", original_time, None);

        // Asset exists and certified_at is preserved.
        assert_eq!(last_certified_at("/posts/1"), Some(original_time));
    }

    // ---- Prefix invalidation tests ----

    #[test]
    fn invalidate_prefix_does_not_over_match() {
        reset_router();
        register_dynamic_path("/posts/1");
        register_dynamic_path("/posts/2");
        register_dynamic_path("/postscript");

        ASSET_ROUTER.with_borrow_mut(|router| {
            let to_remove = router.dynamic_paths_with_prefix("/posts/");
            for p in &to_remove {
                router.delete_asset(p);
            }
        });
        assert!(!is_dynamic_path("/posts/1"));
        assert!(!is_dynamic_path("/posts/2"));
        assert!(
            is_dynamic_path("/postscript"),
            "/postscript should survive /posts/ prefix invalidation"
        );
    }

    #[test]
    fn invalidate_all_dynamic_leaves_empty() {
        reset_router();
        register_dynamic_path("/a");
        register_dynamic_path("/b/c");
        register_dynamic_path("/d/e/f");
        assert_eq!(dynamic_path_count(), 3);

        ASSET_ROUTER.with_borrow_mut(|router| {
            let all = router.dynamic_paths();
            for p in &all {
                router.delete_asset(p);
            }
        });
        assert_eq!(dynamic_path_count(), 0);

        // Subsequent operation is a no-op
        ASSET_ROUTER.with_borrow(|router| {
            assert!(router.dynamic_paths().is_empty());
        });
        assert_eq!(dynamic_path_count(), 0);
    }

    #[test]
    fn invalidate_path_double_removal_is_noop() {
        reset_router();
        register_dynamic_path("/posts/42");
        ASSET_ROUTER.with_borrow_mut(|router| {
            router.delete_asset("/posts/42");
        });
        // Second removal is a no-op (no panic).
        ASSET_ROUTER.with_borrow_mut(|router| {
            router.delete_asset("/posts/42");
        });
        assert!(!is_dynamic_path("/posts/42"));
    }

    // ---- TTL edge cases ----

    #[test]
    fn ttl_one_ns_before_expiry_is_not_expired() {
        reset_router();
        let one_hour_ns: u64 = 3_600_000_000_000;
        register_dynamic_with_ttl(
            "/page",
            1_000_000_000_000_000_000,
            Some(Duration::from_secs(3600)),
        );
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            let now = asset.certified_at + one_hour_ns - 1;
            assert!(
                !asset.is_expired(now),
                "should not be expired 1ns before boundary"
            );
        });
    }

    #[test]
    fn ttl_no_overflow_on_large_values() {
        reset_router();
        register_dynamic_with_ttl("/page", u64::MAX - 1000, Some(Duration::from_secs(3600)));
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            assert!(asset.is_expired(u64::MAX));
            assert!(!asset.is_expired(0));
        });
    }

    #[test]
    fn ttl_zero_duration_immediately_expired() {
        reset_router();
        register_dynamic_with_ttl(
            "/page",
            1_000_000_000_000_000_000,
            Some(Duration::from_secs(0)),
        );
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/page").unwrap();
            assert!(asset.is_expired(asset.certified_at));
            assert!(asset.is_expired(asset.certified_at + 1));
        });
    }

    // ---- NotModified resets certified_at when TTL is active ----

    #[test]
    fn not_modified_resets_certified_at_with_ttl() {
        reset_router();
        let original_time = 1_000_000_000_000_000_000u64;
        let new_time = 2_000_000_000_000_000_000u64;
        register_dynamic_with_ttl("/posts/1", original_time, Some(Duration::from_secs(3600)));

        // Simulate the NotModified TTL reset logic from http_request_update:
        // if asset.ttl.is_some(), reset certified_at to the new time.
        ASSET_ROUTER.with_borrow_mut(|asset_router| {
            if let Some(asset) = asset_router.get_asset_mut("/posts/1") {
                if asset.ttl.is_some() {
                    asset.certified_at = new_time;
                }
            }
        });

        assert_eq!(last_certified_at("/posts/1"), Some(new_time));

        // Verify the TTL is preserved.
        ASSET_ROUTER.with_borrow(|r| {
            let asset = r.get_asset("/posts/1").unwrap();
            assert_eq!(asset.ttl, Some(Duration::from_secs(3600)));
        });
    }
}

use ic_asset_certification::{Asset, AssetConfig, AssetEncoding};
use ic_cdk::api::certified_data_set;
use ic_http_certification::HeaderField;
use include_dir::Dir;

use crate::{mime::get_mime_type, ASSET_ROUTER, ROUTER_CONFIG};

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

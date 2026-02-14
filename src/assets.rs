use ic_asset_certification::{Asset, AssetConfig, AssetEncoding};
use ic_cdk::api::certified_data_set;
use ic_http_certification::HeaderField;
use include_dir::Dir;

use crate::{mime::get_mime_type, ASSET_ROUTER};

pub const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
pub const NO_CACHE_ASSET_CACHE_CONTROL: &str = "public, no-cache, no-store";

// TODO: All this should be configurable in a config file.
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
        assets.push(Asset::new(
            file.path().to_string_lossy().to_string(),
            file.contents().to_vec(),
        ));
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
        let path = file.path().to_string_lossy().to_string();

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

        asset_configs.push(AssetConfig::File {
            path,
            content_type: Some(mime_type.to_string()),
            headers: get_asset_headers(vec![(
                "cache-control".to_string(),
                IMMUTABLE_ASSET_CACHE_CONTROL.to_string(),
            )]),
            fallback_for: vec![],
            aliased_by: vec![],
            encodings: use_encodings,
        });
    }

    for subdir in dir.dirs() {
        collect_assets_with_config(subdir, assets, asset_configs, encodings.clone());
    }
}

// TODO: Should be configurable
pub fn get_asset_headers(additional_headers: Vec<HeaderField>) -> Vec<HeaderField> {
    // set up the default headers and include additional headers provided by the caller
    let mut headers = vec![
        ("strict-transport-security".to_string(), "max-age=31536000; includeSubDomains".to_string()),
        ("x-frame-options".to_string(), "DENY".to_string()),
        ("x-content-type-options".to_string(), "nosniff".to_string()),
        // ("content-security-policy".to_string(), "default-src 'self'; img-src 'self' data:; form-action 'self'; object-src 'none'; frame-ancestors 'none'; upgrade-insecure-requests; block-all-mixed-content".to_string()),
        ("referrer-policy".to_string(), "no-referrer".to_string()),
        ("permissions-policy".to_string(), "accelerometer=(),ambient-light-sensor=(),autoplay=(),battery=(),camera=(),display-capture=(),document-domain=(),encrypted-media=(),fullscreen=(),gamepad=(),geolocation=(),gyroscope=(),layout-animations=(self),legacy-image-formats=(self),magnetometer=(),microphone=(),midi=(),oversized-images=(self),payment=(),picture-in-picture=(),publickey-credentials-get=(),speaker-selection=(),sync-xhr=(self),unoptimized-images=(self),unsized-media=(self),usb=(),screen-wake-lock=(),web-share=(),xr-spatial-tracking=()".to_string()),
        ("cross-origin-embedder-policy".to_string(), "require-corp".to_string()),
        ("cross-origin-opener-policy".to_string(), "same-origin".to_string()),
    ];
    headers.extend(additional_headers);

    headers
}

pub fn delete_assets(asset_paths: Vec<&str>) {
    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        asset_router.delete_assets_by_path(asset_paths);
        certified_data_set(asset_router.root_hash());
    });
}

use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse};
use include_dir::{include_dir, Dir};

pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

static ASSET_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/static");

// ---------------------------------------------------------------------------
// Canister lifecycle
// ---------------------------------------------------------------------------

fn setup() {
    use std::collections::HashMap;
    use std::time::Duration;

    ic_asset_router::set_asset_config(ic_asset_router::AssetConfig {
        cache_config: ic_asset_router::CacheConfig {
            default_ttl: None,
            per_route_ttl: HashMap::from([("/ttl_test".to_string(), Duration::from_secs(5))]),
        },
        ..ic_asset_router::AssetConfig::default()
    });
    ic_asset_router::assets::certify_all_assets(&ASSET_DIR);
}

#[init]
fn init() {
    setup();
}

#[post_upgrade]
fn post_upgrade() {
    setup();
}

// ---------------------------------------------------------------------------
// HTTP interface
// ---------------------------------------------------------------------------

#[query]
fn http_request(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::http_request(
            req,
            routes,
            ic_asset_router::HttpRequestOptions { certify: true },
        )
    })
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| ic_asset_router::http_request_update(req, routes))
}

// ---------------------------------------------------------------------------
// Cache invalidation endpoints (for E2E testing)
// ---------------------------------------------------------------------------

#[update]
fn invalidate(path: String) {
    ic_asset_router::invalidate_path(&path);
}

#[update]
fn invalidate_all() {
    ic_asset_router::invalidate_all_dynamic();
}

// ---------------------------------------------------------------------------
// Cache introspection endpoints (for E2E testing)
// ---------------------------------------------------------------------------

#[query]
fn dynamic_cache_count() -> u64 {
    ic_asset_router::assets::dynamic_path_count() as u64
}

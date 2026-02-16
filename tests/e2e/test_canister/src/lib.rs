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

    router_library::set_asset_config(router_library::AssetConfig {
        cache_config: router_library::CacheConfig {
            default_ttl: None,
            per_route_ttl: HashMap::from([("/ttl_test".to_string(), Duration::from_secs(5))]),
        },
        ..router_library::AssetConfig::default()
    });
    router_library::assets::certify_all_assets(&ASSET_DIR);
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
        router_library::http_request(
            req,
            routes,
            router_library::HttpRequestOptions { certify: true },
        )
    })
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| router_library::http_request_update(req, routes))
}

// ---------------------------------------------------------------------------
// Cache invalidation endpoints (for E2E testing)
// ---------------------------------------------------------------------------

#[update]
fn invalidate(path: String) {
    router_library::invalidate_path(&path);
}

#[update]
fn invalidate_all() {
    router_library::invalidate_all_dynamic();
}

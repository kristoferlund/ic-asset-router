use std::collections::HashMap;
use std::time::Duration;

use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse};

pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

// ---------------------------------------------------------------------------
// Canister lifecycle
// ---------------------------------------------------------------------------

fn setup() {
    router_library::set_asset_config(router_library::AssetConfig {
        security_headers: router_library::SecurityHeaders::permissive(),
        cache_control: router_library::CacheControl::default(),
        cache_config: router_library::CacheConfig {
            // Default TTL: 5 minutes for all dynamic routes.
            default_ttl: Some(Duration::from_secs(300)),
            // Override: the /ttl route has a shorter 30-second TTL.
            per_route_ttl: HashMap::from([("/ttl".to_string(), Duration::from_secs(30))]),
        },
        custom_headers: vec![],
    });
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
// Explicit invalidation endpoints
// ---------------------------------------------------------------------------

/// Invalidate a single cached dynamic asset by path.
///
/// Example: `dfx canister call cache_invalidation invalidate '("/")'`
#[update]
fn invalidate(path: String) {
    router_library::invalidate_path(&path);
}

/// Invalidate all cached dynamic assets.
///
/// Example: `dfx canister call cache_invalidation invalidate_all`
#[update]
fn invalidate_all() {
    router_library::invalidate_all_dynamic();
}

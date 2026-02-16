use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse};

pub mod data;
pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

// ---------------------------------------------------------------------------
// Canister lifecycle
// ---------------------------------------------------------------------------

fn setup() {
    // Permissive security headers (the default) — CORS is handled by our
    // middleware, but permissive headers avoid blocking external API consumers.
    router_library::set_asset_config(router_library::AssetConfig::default());
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

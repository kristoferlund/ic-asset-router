//! API Authentication Example
//!
//! Demonstrates why authenticated endpoints need full certification.
//!
//! - `GET /` — public about page (response-only, default)
//! - `GET /profile` — user profile (authenticated, full certification)
//!
//! Without full certification on `/profile`, a malicious replica could serve
//! Alice's cached profile to Bob. With `#[route(certification = "authenticated")]`,
//! the Authorization header is included in the certification hash, making each
//! user's response independently verifiable.

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
    ic_asset_router::set_asset_config(ic_asset_router::AssetConfig::default());
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

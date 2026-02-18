//! API Authentication Example
//!
//! Demonstrates two patterns for authenticated endpoints:
//!
//! - `GET /` — public about page (response-only, default)
//! - `GET /profile` — user profile (full certification, update call per request)
//! - `GET /customers` — shared customer list (skip certification + handler auth)
//!
//! **Pattern 1 — Full certification (`/profile`):** Each user gets an independently
//! certified response. The `Authorization` header is included in the certification
//! hash, so a malicious replica cannot serve Alice's profile to Bob. Trade-off:
//! every request goes through an update call (~2s).
//!
//! **Pattern 2 — Skip + handler auth (`/customers`):** The handler runs on every
//! query call (like a candid query), checks the `Authorization` header, and returns
//! 401 if missing. The response uses skip certification — identical security model
//! to IC candid query calls. Use this when you need auth checking on every request
//! with fast query-path performance (~200ms).

use ic_asset_router::{HttpRequest, HttpResponse};
use ic_cdk::{init, post_upgrade, query, update};

pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

// ---------------------------------------------------------------------------
// Canister lifecycle
// ---------------------------------------------------------------------------

fn setup() {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::setup(routes).build();
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

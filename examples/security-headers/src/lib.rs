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
        // Demonstrate the strict() preset — the most restrictive configuration.
        // Compare with SecurityHeaders::permissive() or SecurityHeaders::none().
        ic_asset_router::setup(routes)
            .with_config(ic_asset_router::AssetConfig {
                security_headers: ic_asset_router::SecurityHeaders::strict(),
                ..ic_asset_router::AssetConfig::default()
            })
            .build();
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

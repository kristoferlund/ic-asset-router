pub mod data;
pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

use ic_asset_router::{
    assets::{certify_all_assets, delete_assets},
    set_asset_config, AssetConfig, HttpRequestOptions,
};
use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse};
use include_dir::{include_dir, Dir};

static ASSETS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../dist");

fn setup() {
    // Static assets (Vite bundles with content hashes): cached 1 year, immutable.
    // Dynamic assets (server-rendered HTML with SEO tags): no cache.
    set_asset_config(AssetConfig {
        cache_control: ic_asset_router::CacheControl {
            dynamic_assets: "public, no-cache, no-store".into(),
            ..ic_asset_router::CacheControl::default()
        },
        ..AssetConfig::default()
    });

    // Certify all pre-built assets produced by Vite (JS, CSS, etc.)
    certify_all_assets(&ASSETS_DIR);

    // Delete the pre-built index.html from the certified asset cache.
    // Page routes (/, /posts/:postId) will be generated dynamically with
    // route-specific SEO meta tags injected via Tera on first request.
    delete_assets(vec!["/"]);
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
pub fn http_request(req: HttpRequest) -> HttpResponse {
    route_tree::ROUTES
        .with(|routes| ic_asset_router::http_request(req, routes, HttpRequestOptions::default()))
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse {
    route_tree::ROUTES.with(|routes| ic_asset_router::http_request_update(req, routes))
}

// ---------------------------------------------------------------------------
// Candid API
// ---------------------------------------------------------------------------

#[query]
fn list_posts() -> Vec<data::Post> {
    data::all_posts()
}

#[query]
fn get_post(id: i64) -> Result<data::Post, String> {
    data::get_post(id).ok_or_else(|| format!("Post {} not found", id))
}

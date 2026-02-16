use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET /ttl_test → returns the current IC time as a string.
/// Used by E2E tests to verify TTL-based cache expiry: a fresh response
/// will have a different timestamp than a cached one.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let now = ic_cdk::api::time();
    let body = format!("{now}");
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

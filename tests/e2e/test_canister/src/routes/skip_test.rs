use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /skip_test → returns "skip ok" with skip certification mode.
/// The response should NOT have an ic-certificate header.
#[ic_asset_router::route(certification = "skip")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(b"skip ok".to_vec()))
        .build()
}

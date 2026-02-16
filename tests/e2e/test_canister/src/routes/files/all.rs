use ic_http_certification::{HttpResponse, StatusCode};
use ic_asset_router::RouteContext;
use std::borrow::Cow;

/// GET /files/* → returns the wildcard capture
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let capture = ctx.wildcard.as_deref().unwrap_or("");

    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(capture.as_bytes().to_vec()))
        .build()
}

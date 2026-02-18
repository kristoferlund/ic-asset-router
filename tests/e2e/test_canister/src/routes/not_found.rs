use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// Custom 404 → returns "custom 404: <path>"
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let path = ctx.url.split_once('?').map_or(ctx.url.as_str(), |(p, _)| p);
    let body = format!("custom 404: {path}");
    HttpResponse::builder()
        .with_status_code(StatusCode::NOT_FOUND)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

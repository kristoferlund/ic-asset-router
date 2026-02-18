use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /method_test → returns "get"
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(b"get".to_vec()))
        .build()
}

/// POST /method_test → returns "post"
pub fn post(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(b"post".to_vec()))
        .build()
}

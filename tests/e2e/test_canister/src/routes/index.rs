use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET / → returns "hello" as text/html
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/html".to_string())])
        .with_body(Cow::<[u8]>::Owned(b"hello".to_vec()))
        .build()
}

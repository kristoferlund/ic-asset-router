use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /auth_test → returns a response certified with the authenticated preset.
/// Full certification includes the Authorization request header so that
/// different auth tokens produce different certified responses.
#[ic_asset_router::route(certification = "authenticated")]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let auth = ctx
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str())
        .unwrap_or("none");
    let body = format!("auth: {auth}");
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

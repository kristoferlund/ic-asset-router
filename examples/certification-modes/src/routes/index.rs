use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

/// GET / — Default route with ResponseOnly certification.
///
/// No `#[route]` attribute is needed. The default certification mode is
/// ResponseOnly, which certifies the response body, status code, and headers
/// but does not include request details in the hash. This is the correct
/// choice for routes where the response depends only on the URL path.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let body = concat!(
        "<h1>Certification Modes Example</h1>",
        "<p>This canister demonstrates the three certification modes:</p>",
        "<ul>",
        "<li><a href=\"/\">/</a> &mdash; Response-only (default, no attribute needed)</li>",
        "<li><a href=\"/public/health\">/public/health</a> &mdash; Skip (no certification)</li>",
        "<li><a href=\"/api/user\">/api/user</a> &mdash; Authenticated (full certification with Authorization header)</li>",
        "<li><a href=\"/content/articles\">/content/articles</a> &mdash; Custom (full certification with query params)</li>",
        "</ul>",
    );
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.as_bytes().to_vec()))
        .build()
}

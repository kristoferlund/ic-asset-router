use ic_asset_router::{route, RouteContext};
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /content/articles — Custom full certification with query parameters.
///
/// This route uses custom certification that includes the `page` and `limit`
/// query parameters in the hash. This means:
///
/// - `/content/articles?page=1` and `/content/articles?page=2` have
///   different certificates.
/// - A malicious replica cannot serve page 1 content when page 2 is
///   requested — the certificate would not verify.
///
/// Use this mode when the response content depends on specific query
/// parameters (pagination, filtering, sorting).
#[route(certification = custom(query_params = ["page", "limit"]))]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let page = ctx.query.get("page").map(|s| s.as_str()).unwrap_or("1");
    let limit = ctx.query.get("limit").map(|s| s.as_str()).unwrap_or("10");

    let body = format!(
        r#"{{"articles":[],"page":{},"limit":{},"mode":"custom_full"}}"#,
        page, limit
    );

    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

use ic_http_certification::{HttpResponse, StatusCode};
use ic_asset_router::RouteContext;
use std::borrow::Cow;

/// GET / — returns the current timestamp.
///
/// This handler is called via `http_request_update`, and the response is
/// cached and certified. Subsequent GET requests return the cached version
/// until the cache is invalidated or the TTL expires.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let now = ic_cdk::api::time();
    let body = format!(
        "Hello! Server time (ns): {now}\n\n\
         This response is cached. Call `invalidate(\"/\")` or `invalidate_all()` \
         to force regeneration."
    );
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

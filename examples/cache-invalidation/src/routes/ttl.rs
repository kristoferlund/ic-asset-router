use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET /ttl — a route whose response expires automatically via TTL.
///
/// The canister is configured with a per-route TTL of 30 seconds for `/ttl`.
/// After 30 seconds, the next GET triggers an update call that regenerates
/// the response.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let now = ic_cdk::api::time();
    let body = format!(
        "TTL demo — server time (ns): {now}\n\n\
         This page has a 30-second TTL. After that, the next request \
         triggers regeneration via an update call."
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

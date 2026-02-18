use ic_asset_router::{route, HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

/// GET /public/health — Skip certification.
///
/// Health check endpoints typically don't need certification because:
/// - The data is publicly verifiable (the canister is either up or down).
/// - Tampering with a health check has no security impact.
/// - Skipping certification reduces latency and cycle cost.
///
/// A malicious replica could return a fake health status, but this is
/// generally acceptable for monitoring endpoints.
#[route(certification = "skip")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(
            br#"{"status":"ok","mode":"skip"}"#.to_vec(),
        ))
        .build()
}

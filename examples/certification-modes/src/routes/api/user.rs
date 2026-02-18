use ic_asset_router::{route, RouteContext};
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /api/user — Authenticated (full) certification.
///
/// The `authenticated` preset includes the `Authorization` request header
/// in the certification hash. This means:
///
/// - Each unique Authorization token produces an independently certified response.
/// - A malicious replica cannot serve Alice's profile to Bob (the certificate
///   would not verify because the Authorization headers differ).
///
/// Use this mode for any endpoint where the response depends on who is
/// making the request.
#[route(certification = "authenticated")]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Extract the Authorization header (if present) for demonstration.
    let auth = ctx.header("authorization").unwrap_or("anonymous");

    let body = format!(
        r#"{{"user":"demo","auth":"{}","mode":"authenticated"}}"#,
        auth
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

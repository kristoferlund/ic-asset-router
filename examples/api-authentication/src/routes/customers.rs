use ic_asset_router::{route, RouteContext};
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /customers — Auth-gated endpoint with skip certification.
///
/// Skip certification means the handler runs on **every query call**, just
/// like a regular candid query call. This gives you:
///
/// - Fast responses (~200ms query calls, not ~2s update calls)
/// - Handler-level auth checking on every request
/// - Dynamic responses (handler output is never cached)
///
/// **Security model (same as candid query calls):**
/// The handler checks the Authorization header and returns 401 if missing.
/// The response is served with a skip certification proof, meaning a
/// malicious replica could theoretically return fake data — but this is
/// the same trust model as any IC query call. If candid query calls are
/// acceptable for your use case, skip certification is equally acceptable.
///
/// **When to use this pattern:**
/// - The endpoint needs auth checking on every call
/// - The response is dynamic or shared among authenticated users
/// - Performance matters (avoid update call overhead)
/// - The data doesn't require cryptographic response verification
///
/// Compare with `/profile` which uses `#[route(certification = "authenticated")]`
/// — there, each user gets a cryptographically certified response, but every
/// request goes through a slower update call (~2s).
#[route(certification = "skip")]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Auth check runs on EVERY request (query path, not cached).
    let auth = ctx
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str());

    if auth.is_none() {
        return HttpResponse::builder()
            .with_status_code(StatusCode::UNAUTHORIZED)
            .with_headers(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )])
            .with_body(Cow::<[u8]>::Owned(
                br#"{"error":"Missing Authorization header","hint":"Try: curl -H 'Authorization: Bearer any-token' <url>/customers"}"#.to_vec(),
            ))
            .build();
    }

    // Same response for all authenticated users — a shared resource.
    let body = r#"{"customers":[{"id":1,"name":"Acme Corp"},{"id":2,"name":"Globex Inc"}]}"#;

    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.as_bytes().to_vec()))
        .build()
}

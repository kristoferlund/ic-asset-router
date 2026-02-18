use ic_asset_router::{route, RouteContext};
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /profile — Authenticated endpoint with full certification.
///
/// The `authenticated` preset includes the `Authorization` request header in
/// the certification hash. This means:
///
/// 1. Alice requests `/profile` with `Authorization: Bearer alice-token`.
///    The response is certified with her token in the hash.
///
/// 2. Bob requests `/profile` with `Authorization: Bearer bob-token`.
///    He gets a separately certified response.
///
/// 3. A malicious replica cannot give Alice Bob's profile because the
///    certificate hash includes the Authorization header value — it would
///    not verify.
///
/// Without this attribute, the route would use response-only certification
/// (the default), and a cached response for one user could be served to
/// any other user — a serious security issue for authenticated endpoints.
#[route(certification = "authenticated")]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let auth = ctx.header("authorization");

    let (status, body) = match auth {
        Some(token) => {
            // In a real app, you would validate the token and look up the user.
            // For this example, we derive a "username" from the token.
            let username = token
                .strip_prefix("Bearer ")
                .unwrap_or(token);
            (
                StatusCode::OK,
                format!(
                    r#"{{"profile":{{"name":"{}","authenticated":true}}}}"#,
                    username
                ),
            )
        }
        None => (
            StatusCode::UNAUTHORIZED,
            r#"{"error":"Missing Authorization header","hint":"Try: curl -H 'Authorization: Bearer alice-token' <url>/profile"}"#.to_string(),
        ),
    };

    HttpResponse::builder()
        .with_status_code(status)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET / — Public about page (response-only certification, the default).
///
/// No `#[route]` attribute is needed. Everyone sees the same content, so
/// response-only certification is sufficient. A malicious replica cannot
/// tamper with this response because the body is certified.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let body = concat!(
        "<h1>API Authentication Example</h1>",
        "<p>This canister demonstrates why authentication needs full certification.</p>",
        "<ul>",
        "<li><code>GET /</code> &mdash; This page (response-only, public)</li>",
        "<li><code>GET /profile</code> &mdash; User profile (authenticated)</li>",
        "</ul>",
        "<h2>Try it</h2>",
        "<pre>",
        "# Public page (works without auth):\n",
        "curl http://&lt;canister-id&gt;.localhost:4943/\n\n",
        "# Authenticated profile:\n",
        "curl -H 'Authorization: Bearer alice-token' http://&lt;canister-id&gt;.localhost:4943/profile\n",
        "curl -H 'Authorization: Bearer bob-token' http://&lt;canister-id&gt;.localhost:4943/profile\n",
        "</pre>",
        "<p>With <code>#[route(certification = \"authenticated\")]</code>, Alice and Bob ",
        "receive independently certified responses. A malicious replica cannot serve ",
        "Alice's profile to Bob.</p>",
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

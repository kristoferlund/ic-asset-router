use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

/// GET / — Public about page (response-only certification, the default).
///
/// No `#[route]` attribute is needed. Everyone sees the same content, so
/// response-only certification is sufficient. A malicious replica cannot
/// tamper with this response because the body is certified.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let body = concat!(
        "<h1>API Authentication Example</h1>",
        "<p>This canister demonstrates two patterns for authenticated endpoints ",
        "with different security and performance trade-offs.</p>",
        "<h2>Routes</h2>",
        "<ul>",
        "<li><code>GET /</code> &mdash; This page (response-only, public)</li>",
        "<li><code>GET /profile</code> &mdash; User profile (full certification, update call)</li>",
        "<li><code>GET /customers</code> &mdash; Customer list (skip + handler auth, query call)</li>",
        "</ul>",
        "<h2>Pattern 1: Full certification (/profile)</h2>",
        "<p>Use <code>#[route(certification = \"authenticated\")]</code> when the response ",
        "varies per user. Each caller gets a cryptographically certified response bound to ",
        "their Authorization header. A malicious replica cannot serve Alice's profile to Bob.</p>",
        "<p>Trade-off: every request goes through an update call (~2s).</p>",
        "<pre>",
        "curl -H 'Authorization: Bearer alice-token' /profile\n",
        "curl -H 'Authorization: Bearer bob-token' /profile\n",
        "</pre>",
        "<h2>Pattern 2: Skip + handler auth (/customers)</h2>",
        "<p>Use <code>#[route(certification = \"skip\")]</code> when you need auth checking ",
        "on every call but want fast query-path performance. The handler runs on every ",
        "request (just like a candid query call), checks the Authorization header, and ",
        "returns 401 if missing.</p>",
        "<p>Security model: identical to IC candid query calls. The response is not ",
        "cryptographically certified, so a malicious replica could theoretically return ",
        "fake data. This is the same trust model developers accept when using query calls.",
        "</p>",
        "<pre>",
        "# Authenticated &mdash; returns customer list\n",
        "curl -H 'Authorization: Bearer alice-token' /customers\n",
        "curl -H 'Authorization: Bearer bob-token' /customers\n\n",
        "# No auth &mdash; returns 401\n",
        "curl /customers\n",
        "</pre>",
        "<h2>When to use which?</h2>",
        "<table border='1' cellpadding='6'>",
        "<tr><th>Pattern</th><th>Security</th><th>Performance</th><th>Use when</th></tr>",
        "<tr><td>Full (authenticated)</td><td>Response certified per caller</td>",
        "<td>~2s (update call)</td><td>Response varies per user</td></tr>",
        "<tr><td>Skip + handler auth</td><td>Same as candid query</td>",
        "<td>~200ms (query call)</td><td>Shared data, auth is a gate</td></tr>",
        "</table>",
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

use ic_http_certification::{HttpResponse, StatusCode};
use ic_asset_router::RouteContext;
use std::borrow::Cow;

/// GET /permissive — shows the difference when using permissive headers.
///
/// Note: the global config is set to strict() in this example. This page
/// simply documents what the permissive preset would produce, for comparison.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let html = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Permissive Preset (reference)</title></head>
<body>
<h1>Permissive Preset Reference</h1>
<p>This page lists the headers <code>SecurityHeaders::permissive()</code> would set.</p>
<p>The canister itself uses <code>strict()</code>, so the actual response headers
on this page are the strict set. See <code>/</code> for the live values.</p>
<ul>
  <li>strict-transport-security: max-age=31536000; includeSubDomains</li>
  <li>x-content-type-options: nosniff</li>
  <li>x-frame-options: SAMEORIGIN</li>
  <li>referrer-policy: strict-origin-when-cross-origin</li>
  <li>cross-origin-opener-policy: same-origin-allow-popups</li>
  <li>cross-origin-resource-policy: cross-origin</li>
  <li>x-permitted-cross-domain-policies: none</li>
</ul>
<p><a href="/">Back to strict headers demo</a></p>
</body>
</html>"#;

    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(html.as_bytes().to_vec()))
        .build()
}

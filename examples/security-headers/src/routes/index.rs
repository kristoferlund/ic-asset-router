use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET / — returns an HTML page that displays the response headers.
///
/// The page includes inline JavaScript that fetches itself and renders
/// the security headers the server returned. This lets you see the
/// effect of the SecurityHeaders configuration at a glance.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let html = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Security Headers Demo</title></head>
<body>
<h1>Security Headers Demo</h1>
<p>This canister is configured with <code>SecurityHeaders::strict()</code>.</p>
<h2>Response headers</h2>
<pre id="headers">Loading...</pre>
<script>
fetch(window.location.href).then(r => {
  const lines = [];
  r.headers.forEach((v, k) => lines.push(k + ': ' + v));
  document.getElementById('headers').textContent = lines.sort().join('\n');
});
</script>
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

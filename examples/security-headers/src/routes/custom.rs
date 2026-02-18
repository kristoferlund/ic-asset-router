use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

/// GET /custom — documents how to build a custom SecurityHeaders configuration.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let html = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Custom Headers</title></head>
<body>
<h1>Custom Security Headers</h1>
<p>You can also configure individual fields:</p>
<pre><code>SecurityHeaders {
    hsts: Some("max-age=31536000; includeSubDomains".into()),
    csp: Some("default-src 'self'; script-src 'self'".into()),
    content_type_options: Some("nosniff".into()),
    frame_options: None, // allow iframe embedding
    ..SecurityHeaders::none()
}</code></pre>
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

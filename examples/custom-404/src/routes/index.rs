use ic_http_certification::{HttpResponse, StatusCode};
use ic_asset_router::RouteContext;
use std::borrow::Cow;

/// GET / — a simple home page.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let html = r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Custom 404 Demo</title></head>
<body>
<h1>Home</h1>
<p>This canister has a custom <code>not_found.rs</code> handler.</p>
<p>Try visiting a non-existent page like <a href="/nope">/nope</a>.</p>
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

use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// Custom 404 handler — returns styled HTML instead of the default plain-text response.
///
/// The build script detects `not_found.rs` and registers this handler
/// automatically. Any unmatched path will invoke this handler through the
/// middleware chain.
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let path = &ctx.url;
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>404 — Page Not Found</title>
<style>
  body {{ font-family: system-ui, sans-serif; text-align: center; padding: 4rem 1rem; }}
  h1 {{ font-size: 4rem; margin: 0; }}
  p {{ color: #666; }}
  a {{ color: #0366d6; }}
</style>
</head>
<body>
<h1>404</h1>
<p>The page <code>{path}</code> was not found.</p>
<p><a href="/">Go home</a></p>
</body>
</html>"#
    );

    HttpResponse::builder()
        .with_status_code(StatusCode::NOT_FOUND)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(html.into_bytes()))
        .build()
}

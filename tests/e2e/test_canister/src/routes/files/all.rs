use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET /files/* → returns the wildcard capture
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Extract the path portion of the URL, then strip the "/files/" prefix to
    // get the wildcard capture. The URL may be a full URL or just a path.
    let path = ctx.url.split_once('?').map_or(ctx.url.as_str(), |(p, _)| p);
    let capture = path
        .find("/files/")
        .map(|i| &path[i + "/files/".len()..])
        .unwrap_or("");

    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(capture.as_bytes().to_vec()))
        .build()
}

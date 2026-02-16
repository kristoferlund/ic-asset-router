use ic_http_certification::{HttpRequest, HttpResponse};
use router_library::router::RouteParams;

/// Root-level CORS middleware.
///
/// Adds `Access-Control-Allow-Origin: *` and related CORS headers to every
/// response. For OPTIONS preflight requests, short-circuits with a 204 No Content.
pub fn middleware(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static> {
    let cors_headers = vec![
        ("access-control-allow-origin".to_string(), "*".to_string()),
        (
            "access-control-allow-methods".to_string(),
            "GET, POST, PUT, DELETE, OPTIONS".to_string(),
        ),
        (
            "access-control-allow-headers".to_string(),
            "content-type".to_string(),
        ),
    ];

    // Short-circuit OPTIONS preflight with 204 No Content.
    if req.method().as_str() == "OPTIONS" {
        return HttpResponse::builder()
            .with_status_code(ic_http_certification::StatusCode::NO_CONTENT)
            .with_headers(cors_headers)
            .build();
    }

    let response = next(req, params);

    // Append CORS headers to the handler's response.
    let mut headers = response.headers().to_vec();
    headers.extend(cors_headers);

    HttpResponse::builder()
        .with_status_code(response.status_code())
        .with_headers(headers)
        .with_body(response.body().to_vec())
        .build()
}

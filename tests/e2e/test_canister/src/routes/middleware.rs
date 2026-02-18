use ic_asset_router::{HttpRequest, HttpResponse, RouteParams};

/// Adds X-Test-Middleware header to all responses.
pub fn middleware(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static> {
    let response = next(req, params);

    let mut headers = response.headers().to_vec();
    headers.push(("x-test-middleware".to_string(), "applied".to_string()));

    HttpResponse::builder()
        .with_status_code(response.status_code())
        .with_headers(headers)
        .with_body(response.body().to_vec())
        .build()
}

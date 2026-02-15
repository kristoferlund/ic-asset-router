use ic_http_certification::{HttpRequest, HttpResponse};

use crate::router::RouteParams;

/// A middleware function that wraps route handler execution.
///
/// Middleware can inspect/modify the request before calling `next`, short-circuit
/// by returning a response without calling `next`, or inspect/modify the response
/// after calling `next`.
///
/// # Example
///
/// ```ignore
/// pub fn middleware(
///     req: HttpRequest,
///     params: &RouteParams,
///     next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
/// ) -> HttpResponse<'static> {
///     // Pre-processing: inspect or modify request
///     let mut response = next(req, params);
///     // Post-processing: inspect or modify response
///     response
/// }
/// ```
pub type MiddlewareFn = fn(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static>;

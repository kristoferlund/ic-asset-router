use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

use super::Params;

/// GET /echo/:path → returns the path param as text
pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(ctx.params.path.into_bytes()))
        .build()
}

use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

use super::Params;

/// GET /posts/:postId → returns HTML with post ID
pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let body = format!("<h1>Post {}</h1>", ctx.params.post_id);
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".to_string(), "text/html".to_string())])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

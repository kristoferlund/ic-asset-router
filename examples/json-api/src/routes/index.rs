use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

/// GET / — returns a JSON welcome message with links to the API endpoints.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let body = r#"{"message":"JSON API example","endpoints":["/items","/items/:itemId"]}"#;
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.as_bytes().to_vec()))
        .build()
}

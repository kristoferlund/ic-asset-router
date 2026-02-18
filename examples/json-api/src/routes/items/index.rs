use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use std::borrow::Cow;

use crate::data;

/// GET /items — list all items as a JSON array.
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let items = data::list_items();
    let body = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
        .build()
}

/// POST /items — create a new item from the JSON body.
///
/// Expects `{"name":"..."}`. Returns the created item with its assigned ID.
pub fn post(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let input: Result<data::CreateItem, _> = ctx.json();

    match input {
        Ok(create) => {
            let item = data::create_item(create);
            let body = serde_json::to_string(&item).unwrap();
            HttpResponse::builder()
                .with_status_code(StatusCode::CREATED)
                .with_headers(vec![(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )])
                .with_body(Cow::<[u8]>::Owned(body.into_bytes()))
                .build()
        }
        Err(_) => HttpResponse::builder()
            .with_status_code(StatusCode::BAD_REQUEST)
            .with_headers(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )])
            .with_body(Cow::<[u8]>::Owned(
                br#"{"error":"invalid JSON body"}"#.to_vec(),
            ))
            .build(),
    }
}

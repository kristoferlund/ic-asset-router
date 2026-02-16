use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

/// GET /json → returns {"ok":true} as application/json
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "application/json".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(br#"{"ok":true}"#.to_vec()))
        .build()
}

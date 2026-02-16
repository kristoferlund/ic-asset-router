use std::borrow::Cow;

use askama::Template;
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;

use crate::data::{self, Post};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    posts: &'a [Post],
}

pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let posts = data::all_posts();
    let template = IndexTemplate { posts };

    match template.render() {
        Ok(html) => HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )])
            .with_body(Cow::<[u8]>::Owned(html.into_bytes()))
            .build(),
        Err(_) => HttpResponse::builder()
            .with_status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
            .with_body(Cow::<[u8]>::Owned(b"Template rendering failed".to_vec()))
            .build(),
    }
}

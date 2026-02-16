use std::borrow::Cow;

use askama::Template;
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;

use crate::data;

use super::Params;

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    title: &'a str,
    content: &'a str,
    author: &'a str,
    post_id: &'a str,
}

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;

    let Some(post) = data::find_post(post_id) else {
        return HttpResponse::builder()
            .with_status_code(StatusCode::NOT_FOUND)
            .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
            .with_body(Cow::<[u8]>::Owned(b"Post not found".to_vec()))
            .build();
    };

    let template = PostTemplate {
        title: post.title,
        content: post.content,
        author: post.author,
        post_id,
    };

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

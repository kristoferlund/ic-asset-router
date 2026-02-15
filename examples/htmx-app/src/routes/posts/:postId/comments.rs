use std::borrow::Cow;

use askama::Template;
use ic_http_certification::{HttpRequest, HttpResponse, StatusCode};
use router_library::router::RouteParams;

use crate::data::{self, Comment};

/// HTML fragment — no layout wrapper. Returned as an HTMX partial response.
#[derive(Template)]
#[template(path = "partials/comments.html")]
struct CommentsTemplate<'a> {
    comments: &'a [Comment],
}

pub fn get(_req: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
    let post_id = params.get("postId").map(|s| s.as_str()).unwrap_or("0");
    let comments = data::comments_for_post(post_id);
    let template = CommentsTemplate { comments };

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

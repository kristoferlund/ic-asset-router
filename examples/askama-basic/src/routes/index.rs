use std::borrow::Cow;

use askama::Template;
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};

struct PostSummary {
    id: &'static str,
    title: &'static str,
    author: &'static str,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    posts: &'a [PostSummary],
}

pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let posts = vec![
        PostSummary {
            id: "1",
            title: "First Post",
            author: "Alice",
        },
        PostSummary {
            id: "2",
            title: "Second Post",
            author: "Bob",
        },
    ];

    let template = IndexTemplate { posts: &posts };

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

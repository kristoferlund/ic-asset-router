use std::borrow::Cow;

use askama::Template;
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};

use super::Params;

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    title: &'a str,
    content: &'a str,
    author: &'a str,
}

struct Post {
    title: &'static str,
    content: &'static str,
    author: &'static str,
}

fn load_post(id: &str) -> Post {
    match id {
        "1" => Post {
            title: "First Post",
            content: "This is the content of the first post.",
            author: "Alice",
        },
        "2" => Post {
            title: "Second Post",
            content: "This is the content of the second post.",
            author: "Bob",
        },
        _ => Post {
            title: "Unknown Post",
            content: "No post found with that ID.",
            author: "Unknown",
        },
    }
}

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;
    let post = load_post(post_id);

    let template = PostTemplate {
        title: post.title,
        content: post.content,
        author: post.author,
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

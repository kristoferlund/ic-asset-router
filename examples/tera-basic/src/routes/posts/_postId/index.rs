use std::borrow::Cow;
use std::cell::RefCell;

use ic_http_certification::{HttpResponse, StatusCode};
use ic_asset_router::RouteContext;
use tera::{Context, Tera};

use super::Params;

thread_local! {
    static TERA: RefCell<Tera> = RefCell::new({
        let mut tera = Tera::default();
        tera.add_raw_template("post.html", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/post.html")))
            .expect("failed to add post.html template");
        tera
    });
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

    let mut context = Context::new();
    context.insert("title", post.title);
    context.insert("content", post.content);
    context.insert("author", post.author);

    let result = TERA.with(|t| t.borrow().render("post.html", &context));

    match result {
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

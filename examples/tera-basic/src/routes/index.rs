use std::borrow::Cow;
use std::cell::RefCell;

use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use tera::{Context, Tera};

thread_local! {
    static TERA: RefCell<Tera> = RefCell::new({
        let mut tera = Tera::default();
        tera.add_raw_template("index.html", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/index.html")))
            .expect("failed to add index.html template");
        tera
    });
}

#[derive(serde::Serialize)]
struct PostSummary {
    id: &'static str,
    title: &'static str,
    author: &'static str,
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

    let mut context = Context::new();
    context.insert("posts", &posts);

    let result = TERA.with(|t| t.borrow().render("index.html", &context));

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

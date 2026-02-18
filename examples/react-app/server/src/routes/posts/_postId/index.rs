use std::borrow::Cow;
use std::cell::RefCell;

use crate::data;
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use tera::{Context, Tera};

use super::Params;

thread_local! {
    static TERA: RefCell<Tera> = RefCell::new({
        let mut tera = Tera::default();
        tera.add_raw_template(
            "index.html",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../dist/index.html")),
        )
        .expect("failed to add index.html template");
        tera
    });
}

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let id: i64 = match ctx.params.post_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::builder()
                .with_status_code(StatusCode::BAD_REQUEST)
                .with_headers(vec![("Content-Type".into(), "text/plain".into())])
                .with_body(b"Invalid post ID".to_vec())
                .build();
        }
    };

    let (title, description) = match data::get_post(id) {
        Some(post) => (post.title, post.summary),
        None => (
            "Post Not Found".to_string(),
            "The requested post could not be found.".to_string(),
        ),
    };

    let mut context = Context::new();
    context.insert("title", &title);
    context.insert("description", &description);

    let html = TERA.with(|t| t.borrow().render("index.html", &context));

    match html {
        Ok(rendered) => HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![("Content-Type".into(), "text/html".into())])
            .with_body(Cow::Owned(rendered.into_bytes()))
            .build(),
        Err(_) => HttpResponse::builder()
            .with_status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .with_headers(vec![("Content-Type".into(), "text/plain".into())])
            .with_body(b"Template rendering failed".to_vec())
            .build(),
    }
}

use std::borrow::Cow;
use std::cell::RefCell;

use ic_asset_router::RouteContext;
use ic_http_certification::{HttpResponse, StatusCode};
use tera::{Context, Tera};

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

pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    let mut context = Context::new();
    context.insert("title", "Page Not Found");
    context.insert("description", "The requested page could not be found.");

    let html = TERA.with(|t| t.borrow().render("index.html", &context));

    match html {
        Ok(rendered) => HttpResponse::builder()
            .with_status_code(StatusCode::NOT_FOUND)
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

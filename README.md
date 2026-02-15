# router_library

## Template Engine Integration

The router library's handler API returns `HttpResponse`, which can contain any HTML however it was generated. This makes it straightforward to integrate template engines. The handler sets the `content-type` header and the library respects it through certification.

### Askama (compile-time templates)

Askama compiles templates into Rust code at build time. There is no runtime template parsing and no filesystem access required, making it the natural choice for ICP canisters.

```rust
use askama::Template;
use ic_http_certification::{HttpRequest, HttpResponse, StatusCode};
use router_library::router::RouteParams;
use std::borrow::Cow;

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    title: &'a str,
    content: &'a str,
    author: &'a str,
}

pub fn get(_req: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
    let post_id = params.get("postId").map(|s| s.as_str()).unwrap_or("0");

    let template = PostTemplate {
        title: "My Post",
        content: "Post content here.",
        author: "Alice",
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
            .with_body(b"Template rendering failed" as &[u8])
            .build(),
    }
}
```

Key points:
- Templates live in a `templates/` directory and are embedded at compile time
- Add `askama` as a dependency in your `Cargo.toml`
- Handle template rendering errors gracefully (avoid `.unwrap()` in production)
- Set the `content-type` header in the handler response

See [`examples/askama-basic/`](examples/askama-basic/) for a complete working example.

### Tera (runtime templates)

Tera parses templates at runtime. In a canister, templates must be embedded via `include_str!` at compile time and loaded into the Tera engine during initialization, since canisters have no filesystem access.

```rust
use ic_http_certification::{HttpRequest, HttpResponse, StatusCode};
use router_library::router::RouteParams;
use std::borrow::Cow;
use std::cell::RefCell;
use tera::{Context, Tera};

thread_local! {
    static TERA: RefCell<Tera> = RefCell::new({
        let mut tera = Tera::default();
        tera.add_raw_template("post.html", include_str!("../templates/post.html"))
            .expect("failed to add template");
        tera
    });
}

pub fn get(_req: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
    let mut context = Context::new();
    context.insert("title", "My Post");
    context.insert("content", "Post content here.");
    context.insert("author", "Alice");

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
            .with_body(b"Template rendering failed" as &[u8])
            .build(),
    }
}
```

Key points:
- Load templates via `include_str!` -- canisters have no filesystem at runtime
- Use the `thread_local!` pattern for canister state
- Tera supports template inheritance (`{% extends "layout.html" %}`)
- Adds runtime overhead (template parsing at init) but offers more flexibility

See [`examples/tera-basic/`](examples/tera-basic/) for a complete working example.

### Askama vs Tera

| Aspect | Askama | Tera |
|--------|--------|------|
| Template compilation | Build time | Runtime (at init) |
| Type safety | Full (compile errors for missing vars) | None (runtime errors) |
| Template inheritance | Limited (blocks, includes) | Full (extends, blocks, macros) |
| WASM binary size | Smaller (no parser) | Larger (includes parser) |
| Flexibility | Templates fixed at compile time | Templates can be loaded dynamically |
| Recommendation | Default choice for most canisters | Use when you need full template inheritance or dynamic templates |

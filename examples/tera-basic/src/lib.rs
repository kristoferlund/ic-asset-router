use std::borrow::Cow;
use std::cell::RefCell;

use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse, Method, StatusCode};
use router_library::router::{HandlerFn, NodeType, RouteNode, RouteParams};
use tera::{Context, Tera};

// ---------------------------------------------------------------------------
// Template engine (runtime, loaded via include_str!)
// ---------------------------------------------------------------------------

thread_local! {
    static TERA: RefCell<Tera> = RefCell::new({
        let mut tera = Tera::default();
        tera.add_raw_template("post.html", include_str!("../templates/post.html"))
            .expect("failed to add post.html template");
        tera
    });
}

// ---------------------------------------------------------------------------
// Sample data
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Route handler
// ---------------------------------------------------------------------------

fn get_post(_req: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
    let post_id = params.get("postId").map(|s| s.as_str()).unwrap_or("0");
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

// ---------------------------------------------------------------------------
// Route tree
// ---------------------------------------------------------------------------

fn build_routes() -> RouteNode {
    let mut root = RouteNode::new(NodeType::Static("".into()));
    root.insert("/posts/:postId", Method::GET, get_post as HandlerFn);
    root
}

thread_local! {
    static ROUTES: RouteNode = build_routes();
}

// ---------------------------------------------------------------------------
// Canister lifecycle
// ---------------------------------------------------------------------------

#[init]
fn init() {}

#[post_upgrade]
fn post_upgrade() {}

// ---------------------------------------------------------------------------
// HTTP interface
// ---------------------------------------------------------------------------

#[query]
fn http_request(req: HttpRequest) -> HttpResponse<'static> {
    ROUTES.with(|routes| {
        router_library::http_request(
            req,
            routes,
            router_library::HttpRequestOptions { certify: false },
        )
    })
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse<'static> {
    ROUTES.with(|routes| router_library::http_request_update(req, routes))
}

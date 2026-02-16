# router_library

File-based HTTP router and asset certification library for [Internet Computer](https://internetcomputer.org/) canisters.

**Features:**

- **File-based routing** — place handler files in `src/routes/` and the build script generates a route tree automatically
- **IC response certification** — static and dynamic assets are certified via the IC HTTP certification library
- **Typed route context** — handlers receive `RouteContext<P, S>` with typed path parameters, query parameters, headers, body, and URL
- **Middleware** — scoped middleware functions for cross-cutting concerns (auth, logging, CORS)
- **Security headers** — configurable presets (strict, permissive, none) for standard security response headers
- **Cache control** — configurable `Cache-Control` for static and dynamic assets, plus TTL-based cache invalidation

## Getting Started

### 1. Add the dependency

Add `router_library` and its required IC dependencies to your `Cargo.toml`. The library must appear in both `[dependencies]` (for runtime) and `[build-dependencies]` (for the build script).

```toml
[package]
name = "my-canister"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
candid = "0.10"
ic-cdk = "0.18"
ic-http-certification = "3.0"
router_library = { path = "../router_library" }  # or git/registry

[build-dependencies]
router_library = { path = "../router_library" }
```

### 2. Create the build script

Create `build.rs` in the crate root. This scans `src/routes/` and generates the route tree.

```rust
// build.rs
fn main() {
    router_library::build::generate_routes();
}
```

### 3. Create the routes directory

```
src/
  routes/
    index.rs      # handles GET /
  lib.rs
```

### 4. Write your first route handler

Handlers are Rust files in `src/routes/`. Each file exports one or more public functions named after HTTP methods (`get`, `post`, `put`, `delete`, etc.). Every handler receives a `RouteContext` and returns an `HttpResponse`.

```rust
// src/routes/index.rs
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(b"<h1>Hello from the IC!</h1>".to_vec()))
        .build()
}
```

### 5. Wire up the canister entry points

In `src/lib.rs`, include the generated route tree and expose the IC HTTP interface:

```rust
// src/lib.rs
use ic_cdk::{init, post_upgrade, query, update};
use ic_http_certification::{HttpRequest, HttpResponse};

pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

fn setup() {
    router_library::set_asset_config(router_library::AssetConfig::default());
}

#[init]
fn init() {
    setup();
}

#[post_upgrade]
fn post_upgrade() {
    setup();
}

#[query]
fn http_request(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| {
        router_library::http_request(
            req,
            routes,
            router_library::HttpRequestOptions { certify: true },
        )
    })
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| router_library::http_request_update(req, routes))
}
```

### 6. Add the Candid interface file

Create a `.did` file (e.g. `my_canister.did`) describing the HTTP interface:

```candid
type HeaderField = record { text; text };

type HttpRequest = record {
  method : text;
  url : text;
  headers : vec HeaderField;
  body : blob;
  certificate_version : opt nat16;
};

type HttpResponse = record {
  status_code : nat16;
  headers : vec HeaderField;
  body : blob;
};

service : {
  http_request : (HttpRequest) -> (HttpResponse) query;
  http_request_update : (HttpRequest) -> (HttpResponse);
};
```

### 7. Add `dfx.json`

```json
{
  "canisters": {
    "my_canister": {
      "type": "rust",
      "package": "my-canister",
      "candid": "my_canister.did"
    }
  },
  "defaults": {
    "build": {
      "args": ""
    }
  },
  "version": 1
}
```

### 8. Deploy and test

```sh
dfx start --clean --background
dfx deploy
curl "http://$(dfx canister id my_canister).localhost:4943/"
```

You should see `<h1>Hello from the IC!</h1>`.

## Routing Conventions

### File naming

| Filename | Route | Description |
|----------|-------|-------------|
| `index.rs` | `/` (directory root) | Index handler for the directory |
| `about.rs` | `/about` | Static named route |
| `_postId/index.rs` | `/:postId` | Dynamic parameter segment |
| `all.rs` | `/*` | Catch-all wildcard |
| `middleware.rs` | — | Middleware for the directory (not a route) |
| `not_found.rs` | — | Custom 404 handler (not a route) |

### Dynamic parameters

Prefix a directory name with `_` to create a dynamic parameter segment. The build script generates a typed `Params` struct in the directory's `mod.rs`:

```
src/routes/
  posts/
    _postId/
      index.rs    # handles GET /posts/:postId
```

```rust
// src/routes/posts/_postId/index.rs
use router_library::RouteContext;
use ic_http_certification::HttpResponse;

// `Params` is generated by the build script in the parent mod.rs:
//   pub struct Params { pub post_id: String }
use super::Params;

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;
    // ... build response
    # todo!()
}
```

### Catch-all routes

Name a file `all.rs` to capture the remaining path as a wildcard:

```rust
// src/routes/files/all.rs
use router_library::RouteContext;
use ic_http_certification::HttpResponse;

pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    // The wildcard path is available via ctx.query or route params
    // ...
    # todo!()
}
```

### Route attribute override

Use `#[route(path = "...")]` to override the filename-derived path segment:

```rust
// src/routes/mw_page.rs
#[route(path = "middleware")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Serves GET /middleware without conflicting with the reserved middleware.rs
    # todo!()
}
```

### Reserved filenames

`middleware.rs` and `not_found.rs` have special behavior and are never registered as routes. To serve content at `/middleware` or `/not_found`, use a differently-named file with `#[route(path = "...")]`.

## Typed Search Params

Define a `SearchParams` struct in a route file to get query string parameters deserialized automatically:

```rust
// src/routes/search.rs
use router_library::RouteContext;
use ic_http_certification::HttpResponse;

#[derive(serde::Deserialize, Default)]
pub struct SearchParams {
    pub page: Option<u32>,
    pub filter: Option<String>,
}

pub fn get(ctx: RouteContext<(), SearchParams>) -> HttpResponse<'static> {
    let page = ctx.search.page.unwrap_or(1);
    let filter = ctx.search.filter.as_deref().unwrap_or("all");
    // ... build response
    # todo!()
}
```

Untyped access to query parameters is always available via `ctx.query` (a `HashMap<String, String>`) regardless of whether `SearchParams` is defined.

## Middleware

Place a `middleware.rs` file in any route directory. The middleware function wraps all handlers in that directory and its subdirectories.

```rust
// src/routes/middleware.rs
use ic_http_certification::{HttpRequest, HttpResponse};
use router_library::router::RouteParams;

pub fn middleware(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static> {
    // Pre-processing: inspect or modify the request
    let response = next(req, params);
    // Post-processing: inspect or modify the response
    response
}
```

Middleware at different directory levels composes automatically. Root middleware runs first, then progressively more specific middleware.

## Security Headers

Configure security headers via `AssetConfig`:

```rust
use router_library::{AssetConfig, SecurityHeaders};

fn setup() {
    router_library::set_asset_config(AssetConfig {
        security_headers: SecurityHeaders::strict(),
        ..AssetConfig::default()
    });
}
```

Three presets are available:

- `SecurityHeaders::strict()` — maximum security; blocks cross-origin resources, iframe embedding, DNS prefetch
- `SecurityHeaders::permissive()` — allows cross-origin resources and `SAMEORIGIN` framing (the default)
- `SecurityHeaders::none()` — no security headers; the consumer takes full responsibility

Individual fields can be overridden on any preset:

```rust
let mut headers = SecurityHeaders::strict();
headers.frame_options = Some("SAMEORIGIN".into());
headers.csp = Some("default-src 'self'; script-src 'self'".into());
```

## Cache Control & TTL

Configure cache-control headers and TTL-based cache invalidation:

```rust
use std::collections::HashMap;
use std::time::Duration;
use router_library::{AssetConfig, CacheConfig, CacheControl};

fn setup() {
    router_library::set_asset_config(AssetConfig {
        cache_control: CacheControl {
            static_assets: "public, max-age=31536000, immutable".into(),
            dynamic_assets: "public, no-cache, no-store".into(),
        },
        cache_config: CacheConfig {
            default_ttl: Some(Duration::from_secs(300)),  // 5 minutes
            per_route_ttl: HashMap::from([
                ("/api/status".to_string(), Duration::from_secs(30)),
            ]),
        },
        ..AssetConfig::default()
    });
}
```

Explicit invalidation is available via:

- `router_library::invalidate_path("/posts/1")` — invalidate a single path
- `router_library::invalidate_prefix("/posts/")` — invalidate all paths under a prefix
- `router_library::invalidate_all_dynamic()` — invalidate all dynamic assets

## Examples

| Example | Features demonstrated |
|---------|---------------------|
| [`examples/askama-basic/`](examples/askama-basic/) | Askama template rendering |
| [`examples/tera-basic/`](examples/tera-basic/) | Tera template rendering |
| [`examples/htmx-app/`](examples/htmx-app/) | Full SSR with HTMX, partials, dynamic params |
| [`examples/security-headers/`](examples/security-headers/) | Strict/permissive/custom security header configuration |
| [`examples/json-api/`](examples/json-api/) | JSON endpoints, method routing, CORS middleware |
| [`examples/cache-invalidation/`](examples/cache-invalidation/) | TTL-based expiry, explicit invalidation |
| [`examples/custom-404/`](examples/custom-404/) | Custom `not_found.rs` handler |

## Template Engine Integration

The router library's handler API returns `HttpResponse`, which can contain any HTML however it was generated. This makes it straightforward to integrate template engines. The handler sets the `content-type` header and the library respects it through certification.

### Askama (compile-time templates)

Askama compiles templates into Rust code at build time. There is no runtime template parsing and no filesystem access required, making it the natural choice for ICP canisters.

```rust
use askama::Template;
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
use std::borrow::Cow;

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    title: &'a str,
    content: &'a str,
    author: &'a str,
}

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;

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
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;
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

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
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

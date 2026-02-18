# ic-asset-router

Build full-stack web applications on the [Internet Computer](https://internetcomputer.org/) with the same file-based routing conventions you know from Next.js and SvelteKit — but in Rust, compiled to a single canister. Drop a handler file into `src/routes/`, deploy, and your endpoint is live with automatic response certification, typed parameters, scoped middleware, and configurable security headers. No frontend framework required; bring your own template engine, return JSON, or serve raw HTML.

## Features

- **File-based routing** — `src/routes/` maps directly to URL paths. Dynamic segments (`_postId/`), catch-all wildcards (`all.rs`), and nested directories are all supported.
- **IC response certification** — responses are automatically certified so boundary nodes can verify them. Choose from three certification modes (Skip, ResponseOnly, Full) per route via `#[route(certification = "...")]`. See [Certification Modes](#certification-modes).
- **Typed route context** — handlers receive a [`RouteContext<P, S>`](https://docs.rs/ic-asset-router/latest/ic_asset_router/context/struct.RouteContext.html) with typed path params, typed search params, headers, body, and the full URL.
- **Scoped middleware** — place a `middleware.rs` in any directory to wrap all handlers below it. Middleware composes from root to leaf.
- **Security headers** — choose from [`strict`](https://docs.rs/ic-asset-router/latest/ic_asset_router/config/struct.SecurityHeaders.html#method.strict), [`permissive`](https://docs.rs/ic-asset-router/latest/ic_asset_router/config/struct.SecurityHeaders.html#method.permissive), or [`none`](https://docs.rs/ic-asset-router/latest/ic_asset_router/config/struct.SecurityHeaders.html#method.none) presets, or configure individual headers.
- **Cache control & TTL** — set `Cache-Control` per asset type, configure TTL-based expiry, and invalidate cached responses on demand.

## Table of Contents

- [Quick Start](#quick-start)
- [Route Handlers](#route-handlers)
- [Routing Conventions](#routing-conventions)
- [Middleware](#middleware)
- [Certification Modes](#certification-modes)
- [Configuration](#configuration)
- [Examples](#examples)
- [How This Library Was Built](#how-this-library-was-built)
- [Updates](#updates)
- [Author](#author)
- [Contributing](#contributing)
- [License](#license)

## Quick Start

### 1. Add the dependency

`ic-asset-router` must appear in both `[dependencies]` and `[build-dependencies]`:

```toml
[dependencies]
candid = "0.10"
ic-cdk = "0.18"
ic-asset-router = { path = "../ic-asset-router" }

[build-dependencies]
ic-asset-router = { path = "../ic-asset-router" }
```

### 2. Create the build script

```rust
// build.rs
fn main() {
    ic_asset_router::build::generate_routes();
}
```

### 3. Write a route handler

```rust
// src/routes/index.rs
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
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

### 4. Wire up the canister

```rust
// src/lib.rs
use ic_cdk::{init, post_upgrade, query, update};
use ic_asset_router::{HttpRequest, HttpResponse};

pub mod routes;

mod route_tree {
    include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
}

fn setup() {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::setup(routes).build();
    });
}

#[init]
fn init() { setup(); }

#[post_upgrade]
fn post_upgrade() { setup(); }

#[query]
fn http_request(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::http_request(req, routes, Default::default())
    })
}

#[update]
fn http_request_update(req: HttpRequest) -> HttpResponse<'static> {
    route_tree::ROUTES.with(|routes| ic_asset_router::http_request_update(req, routes))
}
```

## Route Handlers

Each `.rs` file in `src/routes/` is a route handler. Export one or more public functions named after HTTP methods and the build script wires them to the matching URL path automatically.

### Supported methods

Export any combination of these function names from a single file:

| Function | HTTP method |
|----------|-------------|
| `get`    | GET         |
| `post`   | POST        |
| `put`    | PUT         |
| `patch`  | PATCH       |
| `delete` | DELETE      |
| `head`   | HEAD        |
| `options`| OPTIONS     |

Only `pub fn` declarations are detected — private functions are ignored. A file with no recognized public method function causes a build error.

### Handler signature

Every handler receives a [`RouteContext`](https://docs.rs/ic-asset-router/latest/ic_asset_router/context/struct.RouteContext.html) and returns an `HttpResponse<'static>`. All types are re-exported from `ic_asset_router`:

```rust
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".into(), "text/plain".into())])
        .with_body(Cow::<[u8]>::Owned(b"Hello!".to_vec()))
        .build()
}
```

The type parameter `P` in `RouteContext<P>` is the typed params struct generated by the build script for routes with dynamic segments. Use `()` for routes without dynamic segments.

### Multiple methods in one file

A single file can handle several HTTP methods. The library returns `405 Method Not Allowed` with a correct `Allow` header for methods that exist at the same path but weren't requested:

```rust
// src/routes/items/_itemId/index.rs
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    // GET /items/:itemId — retrieve
    // ...
    # todo!()
}

pub fn put(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    // PUT /items/:itemId — update
    // ...
    # todo!()
}

pub fn delete(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    // DELETE /items/:itemId — delete
    // ...
    # todo!()
}

use super::Params; // generated: pub struct Params { pub item_id: String }
```

### What RouteContext provides

Handlers receive all request data through the context object:

| Field | Type | Description |
|-------|------|-------------|
| `ctx.params` | `P` | Typed path parameters (e.g. `ctx.params.post_id`) |
| `ctx.search` | `S` | Typed search (query string) params (default `()`) |
| `ctx.query` | `HashMap<String, String>` | Untyped query params, always available |
| `ctx.method` | `Method` | HTTP method |
| `ctx.headers` | `Vec<(String, String)>` | Request headers |
| `ctx.body` | `Vec<u8>` | Raw request body |
| `ctx.url` | `String` | Full request URL |
| `ctx.wildcard` | `Option<String>` | Catch-all wildcard tail |

Convenience methods: `ctx.header("name")`, `ctx.body_to_str()`, `ctx.json::<T>()`, `ctx.form::<T>()`, `ctx.form_data()`.

See the [`json-api`](examples/json-api/) example for a complete REST API with GET, POST, PUT, and DELETE.

## Routing Conventions

| Pattern | Route | Description |
|---------|-------|-------------|
| `index.rs` | `/` | Index handler for the enclosing directory |
| `about.rs` | `/about` | Named route |
| `og.png.rs` | `/og.png` | Dotted filename — serves at the literal path including the extension |
| `_postId/index.rs` | `/:postId` | Dynamic segment — generates a typed `Params` struct |
| `all.rs` | `/*` | Catch-all wildcard — remaining path in `ctx.wildcard` |
| `middleware.rs` | — | Wraps all handlers in this directory and below |
| `not_found.rs` | — | Custom 404 handler |

### Dynamic parameters

Prefix a directory with `_` to capture a path segment. The build script generates a `Params` struct automatically:

```rust
// src/routes/posts/_postId/index.rs
use super::Params; // generated: pub struct Params { pub post_id: String }

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;
    // ...
}
```

### Dotted filenames

Name a source file `og.png.rs` and the handler serves at the URL path `og.png` — the `.rs` extension is stripped but all other dots are preserved. A request to `/app/42/og.png` hits the handler in `src/routes/app/_id/og.png.rs`. This is useful for dynamically generated assets like images or feeds that need a specific file extension in the URL:

```rust
// src/routes/app/_id/og.png.rs → serves at /app/:id/og.png
pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let png_bytes = generate_og_image(&ctx.params.id);
    HttpResponse::builder()
        .with_status_code(StatusCode::OK)
        .with_headers(vec![("content-type".into(), "image/png".into())])
        .with_body(Cow::Owned(png_bytes))
        .build()
}
```

Under the hood, the build script converts dots to underscores for the Rust module name (`og.png.rs` → `mod og_png`) and emits a `#[path = "og.png.rs"]` attribute so the compiler can find the source file.

### Typed search params

Define a `SearchParams` struct in a route file and the query string is deserialized into `ctx.search`:

```rust
#[derive(serde::Deserialize, Default)]
pub struct SearchParams {
    pub page: Option<u32>,
    pub filter: Option<String>,
}

pub fn get(ctx: RouteContext<(), SearchParams>) -> HttpResponse<'static> {
    let page = ctx.search.page.unwrap_or(1);
    // ...
}
```

Untyped query params are always available via `ctx.query`.

### Middleware

Place a `middleware.rs` file in any directory under `src/routes/` and it wraps every handler in that directory and all subdirectories below it. The file must export a `pub fn middleware` with this signature:

```rust
// src/routes/middleware.rs
use ic_asset_router::{HttpRequest, HttpResponse, RouteParams};

pub fn middleware(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static> {
    // Before: inspect or modify the request
    let response = next(req, params);
    // After: inspect or modify the response
    response
}
```

Middleware can:

- **Modify the request** — `req` is owned; construct or alter it before passing to `next`.
- **Modify the response** — capture the return value of `next` and transform headers, body, or status before returning.
- **Short-circuit** — return a response without calling `next` at all (e.g. return 401 for unauthorized requests). The handler never executes.

#### Composition order

Middleware at different directory levels composes automatically in root-to-leaf order. For a request to `/api/v2/data`:

```
root middleware → /api middleware → /api/v2 middleware → handler
```

On the way back, responses unwind in reverse (onion model). Only one middleware per directory is allowed.

Middleware also wraps the custom 404 handler — root-level middleware runs before `not_found.rs`.

#### Example: CORS middleware

```rust
// src/routes/middleware.rs
use ic_asset_router::{HttpRequest, HttpResponse, RouteParams, StatusCode};

pub fn middleware(
    req: HttpRequest,
    params: &RouteParams,
    next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
) -> HttpResponse<'static> {
    let cors_headers = vec![
        ("access-control-allow-origin".into(), "*".into()),
        ("access-control-allow-methods".into(), "GET, POST, PUT, DELETE, OPTIONS".into()),
        ("access-control-allow-headers".into(), "content-type".into()),
    ];

    // Short-circuit: respond to OPTIONS preflight without running the handler
    if req.method().as_str() == "OPTIONS" {
        return HttpResponse::builder()
            .with_status_code(StatusCode::NO_CONTENT)
            .with_headers(cors_headers)
            .build();
    }

    // Call the handler chain
    let response = next(req, params);

    // Append CORS headers to the response
    let mut headers = response.headers().to_vec();
    headers.extend(cors_headers);
    HttpResponse::builder()
        .with_status_code(response.status_code())
        .with_headers(headers)
        .with_body(response.body().to_vec())
        .build()
}
```

See the [`json-api`](examples/json-api/) example for a working CORS middleware.

### Catch-all wildcards

Name a file `all.rs` to capture the entire remaining path. The matched tail is available via `ctx.wildcard`:

```rust
// src/routes/files/all.rs
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let file_path = ctx.wildcard.as_deref().unwrap_or(""); // e.g. "docs/intro.md"
    // ...
}
```

A request to `/files/docs/intro.md` matches the wildcard and `ctx.wildcard` contains `Some("docs/intro.md")`. See [`examples/custom-404`](examples/custom-404/) for a working example.

### Custom 404 handler

Place a `not_found.rs` file at the routes root (or in a subdirectory) to handle requests that don't match any route. The handler has the same signature as a regular route handler:

```rust
// src/routes/not_found.rs
use ic_asset_router::{HttpResponse, RouteContext, StatusCode};
use std::borrow::Cow;

pub fn handler(ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::NOT_FOUND)
        .with_headers(vec![(
            "content-type".to_string(),
            "text/html; charset=utf-8".to_string(),
        )])
        .with_body(Cow::<[u8]>::Owned(b"<h1>Page not found</h1>".to_vec()))
        .build()
}
```

Without a custom `not_found.rs`, the library returns a plain-text 404 response. All 404 responses are certified under a single canonical path to prevent memory growth from bot scans. See [`examples/custom-404`](examples/custom-404/) for a working example.

### Route attribute override

Use `#[route(path = "...")]` to override the filename-derived segment. Useful for serving content at reserved names like `/middleware`:

```rust
// src/routes/mw_page.rs
#[route(path = "middleware")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> { /* ... */ }
```

## Certification Modes

Every HTTP response served by an IC canister can be cryptographically certified so boundary nodes can verify it was not tampered with. This library supports three certification modes, configurable per-route via the `#[route]` attribute:

### Choosing a mode

| Mode | When to use | Example routes |
|------|-------------|----------------|
| **Response-only** (default) | Same URL always returns same content | Static pages, blog posts, docs |
| **Skip** | Tampering has no security impact | Health checks, `/ping` |
| **Skip + handler auth** | Fast auth-gated API (query-path perf) | `/api/customers`, `/api/me` |
| **Authenticated** | Response depends on caller identity, must be tamper-proof | User profiles, dashboards |
| **Custom (Full)** | Response depends on specific headers/params | Content negotiation, pagination |

**Start with the default** (response-only). It requires no configuration and is correct for 90% of routes.

### Response-only (default — no attribute needed)

```rust
// Just write your handler — ResponseOnly is automatic
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_body(b"Hello!" as &[u8])
        .build()
}
```

### Skip certification

```rust
#[route(certification = "skip")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Handler runs on every query call — like a candid query
    HttpResponse::builder()
        .with_body(b"{\"status\":\"ok\"}" as &[u8])
        .build()
}
```

**Handler execution:** Skip-mode routes run the handler on every query call, just like candid `query` calls. This makes them ideal for auth-gated API endpoints — combine with handler-level auth (JWT validation, `ic_cdk::caller()` checks) for fast (~200ms) authenticated queries without waiting for consensus (~2s update calls).

> **Security note:** Skip certification provides the same trust level as candid query calls — both trust the responding replica without cryptographic verification by the boundary node. If candid queries are acceptable for your application, skip certification is equally acceptable.

#### Skip + handler auth pattern

```rust
#[route(certification = "skip")]
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let caller = ic_cdk::caller();
    if caller == Principal::anonymous() {
        return HttpResponse::builder()
            .with_status_code(StatusCode::UNAUTHORIZED)
            .with_body(b"unauthorized" as &[u8])
            .build();
    }
    // Return caller-specific data
    HttpResponse::builder()
        .with_body(format!("hello {caller}").into_bytes())
        .build()
}
```

See the [`api-authentication`](examples/api-authentication/) example for a complete demonstration of both patterns.

### Authenticated (full certification preset)

```rust
#[route(certification = "authenticated")]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Authorization header is included in certification
    // User A cannot receive User B's cached response
    HttpResponse::builder()
        .with_body(b"{\"name\":\"Alice\"}" as &[u8])
        .build()
}
```

### Custom full certification

```rust
#[route(certification = custom(
    request_headers = ["accept"],
    query_params = ["page", "limit"]
))]
pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
    // Each combination of Accept + page + limit is independently certified
    HttpResponse::builder()
        .with_body(b"page content" as &[u8])
        .build()
}
```

### Setup with static assets

Configure the asset router and certify static assets in a single builder chain during `init`/`post_upgrade`:

```rust
use include_dir::{include_dir, Dir};

static ASSET_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets");

fn setup() {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::setup(routes)
            .with_assets(&ASSET_DIR)
            .build();
    });
}
```

For different certification modes per directory:

```rust
use ic_asset_router::CertificationMode;

fn setup() {
    route_tree::ROUTES.with(|routes| {
        ic_asset_router::setup(routes)
            .with_assets(&STATIC_DIR)                                   // response-only (default)
            .with_assets_certified(&PUBLIC_DIR, CertificationMode::skip()) // skip
            .build();
    });
}
```

See the [`certification-modes`](examples/certification-modes/) and [`api-authentication`](examples/api-authentication/) examples for complete, deployable demonstrations.

### Security model: certification vs candid calls

IC canisters support two HTTP interfaces and two candid call types, each with different trust assumptions:

| Mechanism | Consensus | Boundary node verifies? | Trust model |
|-----------|-----------|------------------------|-------------|
| Candid **update** call | Yes (~2s) | N/A | Consensus — response reflects agreed-upon state |
| Candid **query** call | No (~200ms) | No | Trust the replica |
| HTTP + **ResponseOnly/Full** cert | Yes (~2s) | Yes | Consensus — boundary node verifies the certificate |
| HTTP + **Skip** cert | No (~200ms) | No | Trust the replica |

**Key insight:** Skip certification and candid query calls have the same trust model. Both execute on a single replica without consensus, and neither response is cryptographically verified. If your application already uses candid queries (as most IC apps do), skip certification is equally acceptable for equivalent operations.

## Configuration

### Security headers

```rust
ic_asset_router::setup(routes)
    .with_config(ic_asset_router::AssetConfig {
        security_headers: ic_asset_router::SecurityHeaders::strict(),
        ..ic_asset_router::AssetConfig::default()
    })
    .build();
```

Individual fields can be overridden on any preset. See [`SecurityHeaders`](https://docs.rs/ic-asset-router/latest/ic_asset_router/config/struct.SecurityHeaders.html) for all available fields.

### Cache control & invalidation

```rust
use std::collections::HashMap;
use std::time::Duration;
use ic_asset_router::{AssetConfig, CacheConfig, CacheControl};

ic_asset_router::setup(routes)
    .with_config(AssetConfig {
        cache_control: CacheControl {
            static_assets: "public, max-age=31536000, immutable".into(),
            dynamic_assets: "public, no-cache, no-store".into(),
        },
        cache_config: CacheConfig {
            default_ttl: Some(Duration::from_secs(300)),
            per_route_ttl: HashMap::from([
                ("/api/status".to_string(), Duration::from_secs(30)),
            ]),
        },
        ..AssetConfig::default()
    })
    .build();
```

Programmatic invalidation:

- [`invalidate_path`](https://docs.rs/ic-asset-router/latest/ic_asset_router/fn.invalidate_path.html) — single path
- [`invalidate_prefix`](https://docs.rs/ic-asset-router/latest/ic_asset_router/fn.invalidate_prefix.html) — all paths under a prefix
- [`invalidate_all_dynamic`](https://docs.rs/ic-asset-router/latest/ic_asset_router/fn.invalidate_all_dynamic.html) — all dynamic assets

## Examples

Each example is a complete, deployable ICP canister. Clone the repo and `dfx deploy` from any example directory.

| Example | Description |
|---------|-------------|
| [`askama-basic`](examples/askama-basic/) | Compile-time HTML templates with Askama |
| [`tera-basic`](examples/tera-basic/) | Runtime HTML templates with Tera |
| [`htmx-app`](examples/htmx-app/) | Server-rendered blog with HTMX partial updates and static assets |
| [`json-api`](examples/json-api/) | RESTful JSON API with CRUD, method routing, and CORS middleware |
| [`security-headers`](examples/security-headers/) | Security header presets: strict, permissive, and custom |
| [`cache-invalidation`](examples/cache-invalidation/) | TTL-based cache expiry and explicit invalidation |
| [`custom-404`](examples/custom-404/) | Styled 404 page via `not_found.rs` |
| [`certification-modes`](examples/certification-modes/) | Skip, response-only, authenticated, and custom certification modes |
| [`api-authentication`](examples/api-authentication/) | Why authenticated endpoints need full certification |
| [`react-app`](examples/react-app/) | React SPA with TanStack Router/Query, per-route SEO meta tags, and canister API calls |

## How This Library Was Built

> [!NOTE]
> This project was built using the [RALPH loop](https://ghuntley.com/loop) technique: detailed specs for every feature, an implementation plan divided into phases, and a `loop.sh` script that feeds each phase to an AI builder agent one session at a time — keeping the context window focused for maximum output quality. Read more in [RALPH.md](RALPH.md) or browse the [full specs](specs/README.md).

## Updates

See the [CHANGELOG](CHANGELOG.md) for details on updates.

## Author

kristofer@kristoferlund.se

- Twitter: [@kristoferlund](https://twitter.com/kristoferlund)
- Discord: kristoferkristofer
- Telegram: [@kristoferkristofer](https://t.me/kristoferkristofer)

## Contributing

Contributions are welcome. Please submit your pull requests or open issues to propose changes or report bugs.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more details.

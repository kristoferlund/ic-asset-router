//! Build full-stack web applications on the Internet Computer with file-based
//! routing conventions familiar from Next.js and SvelteKit — but in Rust,
//! compiled to a single canister. Drop a handler file into `src/routes/`,
//! deploy, and your endpoint is live with automatic response certification,
//! typed parameters, scoped middleware, and configurable security headers.
//!
//! # Features
//!
//! - **File-based routing** — `src/routes/` maps directly to URL paths.
//!   Dynamic segments (`_postId/`), catch-all wildcards (`all.rs`), dotted
//!   filenames (`og.png.rs` → `/og.png`), and nested directories all work
//!   out of the box. See [`build::generate_routes`].
//! - **IC response certification** — responses are automatically certified so
//!   boundary nodes can verify them. Static assets and dynamic content are
//!   handled transparently.
//! - **Typed route context** — handlers receive a [`RouteContext`] with typed
//!   path params, typed search params, headers, body, and the full URL.
//! - **Scoped middleware** — place a `middleware.rs` in any directory to wrap
//!   all handlers below it. Middleware composes from root to leaf.
//!   See [`middleware::MiddlewareFn`].
//! - **Catch-all wildcards** — name a file `all.rs` to capture the remaining
//!   path. The matched tail is available as `ctx.params["*"]`.
//! - **Custom 404 handler** — place a `not_found.rs` at the routes root to
//!   serve a styled error page instead of the default plain-text 404.
//! - **Security headers** — choose from [`SecurityHeaders::strict`],
//!   [`SecurityHeaders::permissive`], or [`SecurityHeaders::none`] presets,
//!   or configure individual headers.
//! - **Cache control & TTL** — set `Cache-Control` per asset type, configure
//!   TTL-based expiry via [`CacheConfig`], and invalidate cached responses
//!   with [`invalidate_path`], [`invalidate_prefix`], or
//!   [`invalidate_all_dynamic`].
//!
//! # Quick Start
//!
//! **1. Build script** — scans `src/routes/` and generates the route tree:
//!
//! ```rust,ignore
//! // build.rs
//! fn main() {
//!     ic_asset_router::build::generate_routes();
//! }
//! ```
//!
//! **2. Route handler** — a file in `src/routes/` with public `get`, `post`,
//! etc. functions:
//!
//! ```rust,ignore
//! // src/routes/index.rs
//! use ic_http_certification::{HttpResponse, StatusCode};
//! use ic_asset_router::RouteContext;
//! use std::borrow::Cow;
//!
//! pub fn get(_ctx: RouteContext<()>) -> HttpResponse<'static> {
//!     HttpResponse::builder()
//!         .with_status_code(StatusCode::OK)
//!         .with_headers(vec![(
//!             "content-type".to_string(),
//!             "text/html; charset=utf-8".to_string(),
//!         )])
//!         .with_body(Cow::<[u8]>::Owned(b"<h1>Hello from the IC!</h1>".to_vec()))
//!         .build()
//! }
//! ```
//!
//! **3. Canister wiring** — include the generated route tree and expose the
//! HTTP interface:
//!
//! ```rust,ignore
//! // src/lib.rs
//! mod route_tree {
//!     include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"));
//! }
//!
//! fn setup() {
//!     ic_asset_router::set_asset_config(ic_asset_router::AssetConfig::default());
//! }
//! ```
//!
//! See the [`examples/`](https://github.com/kristoferlund/ic-asset-router/tree/main/examples)
//! directory for complete, deployable canister projects including a
//! [React SPA](https://github.com/kristoferlund/ic-asset-router/tree/main/examples/react-app)
//! with TanStack Router/Query and per-route SEO meta tags.

/// Debug logging macro gated behind the `debug-logging` feature flag.
/// When enabled, expands to `ic_cdk::println!`; otherwise compiles to nothing.
#[cfg(feature = "debug-logging")]
macro_rules! debug_log {
    ($($arg:tt)*) => { ic_cdk::println!($($arg)*) };
}

#[cfg(not(feature = "debug-logging"))]
macro_rules! debug_log {
    ($($arg:tt)*) => {};
}

use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use assets::{get_asset_headers, CachedDynamicAsset};
use ic_asset_certification::{
    Asset, AssetConfig as IcAssetConfig, AssetFallbackConfig, AssetRouter,
};
use ic_cdk::api::{certified_data_set, data_certificate};
use ic_http_certification::{
    utils::add_v2_certificate_header, HttpCertification, HttpCertificationPath,
    HttpCertificationTree, HttpCertificationTreeEntry, HttpRequest, HttpResponse, Method,
    StatusCode,
};
use router::{RouteNode, RouteResult};

/// Canonical path used to cache the single certified 404 response.
///
/// All not-found responses are certified and cached under this one path
/// instead of per-request-path, preventing memory growth from bot scans.
const NOT_FOUND_CANONICAL_PATH: &str = "/__not_found";

/// Extract the `content-type` header value from an HTTP response.
///
/// Performs a case-insensitive search for the `content-type` header.
/// Returns `"application/octet-stream"` if no content-type header is present.
fn extract_content_type(response: &HttpResponse) -> String {
    response
        .headers()
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

/// Build a 405 Method Not Allowed response with an `Allow` header listing the
/// permitted methods for the requested path.
fn method_not_allowed(allowed: &[Method]) -> HttpResponse<'static> {
    let allow = allowed
        .iter()
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    HttpResponse::builder()
        .with_status_code(StatusCode::METHOD_NOT_ALLOWED)
        .with_headers(vec![
            ("allow".to_string(), allow),
            ("content-type".to_string(), "text/plain".to_string()),
        ])
        .with_body(Cow::<[u8]>::Owned(b"Method Not Allowed".to_vec()))
        .build()
}

/// Build a plain-text error response for the given HTTP status code and message.
///
/// This avoids canister traps by returning a well-formed HTTP response instead
/// of panicking on malformed input or missing internal state.
fn error_response(status: u16, message: &str) -> HttpResponse<'static> {
    HttpResponse::builder()
        .with_status_code(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
        .with_body(Cow::<[u8]>::Owned(message.as_bytes().to_vec()))
        .build()
}

/// Custom asset router with per-asset certification modes.
pub mod asset_router;
/// Static and dynamic asset certification, invalidation, and serving helpers.
pub mod assets;
/// Build-script utilities for file-based route generation.
pub mod build;
/// Certification mode configuration types.
pub mod certification;
/// Global configuration types: security headers, cache control, TTL settings.
pub mod config;
/// Request context types passed to route handlers.
pub mod context;
/// Middleware type definition.
pub mod middleware;
/// MIME type detection from file extensions.
pub mod mime;
/// Route trie, handler types, and dispatch logic.
pub mod router;

pub use assets::{invalidate_all_dynamic, invalidate_path, invalidate_prefix, last_certified_at};
pub use certification::{CertificationMode, FullConfig, FullConfigBuilder, ResponseOnlyConfig};
pub use config::{AssetConfig, CacheConfig, CacheControl, SecurityHeaders};
pub use context::{
    deserialize_search_params, parse_form_body, parse_query, url_decode, QueryParams, RouteContext,
};
pub use router::HandlerResult;

thread_local! {
    static HTTP_TREE: Rc<RefCell<HttpCertificationTree>> = Default::default();
    static ASSET_ROUTER: RefCell<AssetRouter<'static>> = RefCell::new(AssetRouter::with_tree(HTTP_TREE.with(|tree| tree.clone())));
    static ROUTER_CONFIG: RefCell<AssetConfig> = RefCell::new(AssetConfig::default());
    /// Tracks dynamically generated assets with their certification metadata.
    /// Maps path → `CachedDynamicAsset` (timestamp + optional TTL).
    /// Used by the invalidation API and TTL-based cache expiry.
    static DYNAMIC_CACHE: RefCell<HashMap<String, CachedDynamicAsset>> = RefCell::new(HashMap::new());
}

/// Set the global router configuration.
///
/// Call this during canister initialization (e.g. in `init` or `post_upgrade`)
/// before certifying any assets.
pub fn set_asset_config(config: AssetConfig) {
    ROUTER_CONFIG.with(|c| {
        *c.borrow_mut() = config;
    });
}

/// Options controlling the behavior of [`http_request`].
pub struct HttpRequestOptions {
    /// Whether to attempt serving a certified response from the asset router.
    ///
    /// When `true` (the default), the library checks the asset router for a
    /// previously certified response and returns it directly if available.
    /// When `false`, the handler runs on every request and the response is
    /// served with a skip-certification proof.
    pub certify: bool,
}

impl Default for HttpRequestOptions {
    fn default() -> Self {
        HttpRequestOptions { certify: true }
    }
}

/// Handle an HTTP query-path request.
///
/// This is the IC `http_request` entry point. It resolves the incoming
/// request against the route tree and either:
///
/// 1. Serves a previously certified response from the asset router, or
/// 2. Upgrades the request to an update call (returns `upgrade: true`) so
///    that the handler can run in `http_request_update` and certify a new
///    response.
///
/// Non-GET/HEAD requests are always upgraded. GET requests for dynamic
/// routes with expired TTLs are also upgraded.
pub fn http_request(
    req: HttpRequest,
    root_route_node: &RouteNode,
    opts: HttpRequestOptions,
) -> HttpResponse<'static> {
    debug_log!("http_request: {:?}", req.url());

    let path = match req.get_path() {
        Ok(p) => p,
        Err(_) => return error_response(400, "Bad Request: malformed URL"),
    };

    let method = req.method().clone();

    // Non-GET requests arriving at the query endpoint must be upgraded to an
    // update call so that state-mutating handlers execute in the update path.
    if method != Method::GET && method != Method::HEAD {
        debug_log!(
            "upgrading non-GET request ({}) to update call",
            method.as_str()
        );
        return HttpResponse::builder().with_upgrade(true).build();
    }

    match root_route_node.resolve(&path, &method) {
        RouteResult::Found(handler, params, _result_handler) => match opts.certify {
            false => {
                debug_log!("Serving {} without certification", path);
                let mut response =
                    root_route_node.execute_with_middleware(&path, handler, req, params);

                HTTP_TREE.with(|tree| {
                    let tree = tree.borrow();

                    let cert = match data_certificate() {
                        Some(c) => c,
                        None => {
                            return error_response(
                                500,
                                "Internal Server Error: no data certificate available",
                            );
                        }
                    };

                    let tree_path = HttpCertificationPath::exact(&path);
                    let certification = HttpCertification::skip();
                    let tree_entry = HttpCertificationTreeEntry::new(&tree_path, certification);

                    let witness = match tree.witness(&tree_entry, &path) {
                        Ok(w) => w,
                        Err(_) => {
                            return error_response(
                                500,
                                "Internal Server Error: failed to create certification witness",
                            );
                        }
                    };

                    add_v2_certificate_header(
                        &cert,
                        &mut response,
                        &witness,
                        &tree_path.to_expr_path(),
                    );

                    response
                })
            }
            true => {
                // Check DYNAMIC_CACHE to determine cache state for this path.
                // Three outcomes:
                //   Missing  — path was never generated or was invalidated; upgrade.
                //   Expired  — TTL elapsed; upgrade to regenerate.
                //   Valid    — serve from asset router.
                //
                // We must check DYNAMIC_CACHE *before* calling serve_asset()
                // because the AssetRouter may have a /__not_found fallback
                // registered for scope "/". If the exact asset was deleted
                // (via invalidate_path), serve_asset() would match the
                // fallback and incorrectly return a 404 instead of upgrading.
                enum CacheState {
                    Missing,
                    Expired,
                    Valid,
                }

                let cache_state = DYNAMIC_CACHE.with(|dc| {
                    let cache = dc.borrow();
                    match cache.get(&path) {
                        Some(entry) => {
                            let effective_ttl = entry.ttl.or_else(|| {
                                ROUTER_CONFIG.with(|c| c.borrow().cache_config.effective_ttl(&path))
                            });
                            if let Some(ttl) = effective_ttl {
                                let now_ns = ic_cdk::api::time();
                                let expiry_ns =
                                    entry.certified_at.saturating_add(ttl.as_nanos() as u64);
                                if now_ns >= expiry_ns {
                                    return CacheState::Expired;
                                }
                            }
                            CacheState::Valid
                        }
                        None => CacheState::Missing,
                    }
                });

                match cache_state {
                    CacheState::Missing => {
                        debug_log!("upgrading (not in dynamic cache: {})", path);
                        HttpResponse::builder().with_upgrade(true).build()
                    }
                    CacheState::Expired => {
                        debug_log!("upgrading (TTL expired for {})", path);
                        HttpResponse::builder().with_upgrade(true).build()
                    }
                    CacheState::Valid => ASSET_ROUTER.with_borrow(|asset_router| {
                        let cert = match data_certificate() {
                            Some(c) => c,
                            None => {
                                debug_log!("upgrading (no data certificate)");
                                return HttpResponse::builder().with_upgrade(true).build();
                            }
                        };
                        if let Ok(response) = asset_router.serve_asset(&cert, &req) {
                            debug_log!("serving directly");
                            response
                        } else {
                            debug_log!("upgrading");
                            HttpResponse::builder().with_upgrade(true).build()
                        }
                    }),
                }
            }
        },
        RouteResult::MethodNotAllowed(allowed) => method_not_allowed(&allowed),
        RouteResult::NotFound => {
            if opts.certify {
                // Check DYNAMIC_CACHE for the canonical /__not_found entry.
                // All 404 responses are certified under this single path to
                // prevent memory growth from bot scans.
                let canonical_state = DYNAMIC_CACHE.with(|dc| {
                    let cache = dc.borrow();
                    cache.get(NOT_FOUND_CANONICAL_PATH).map(|entry| {
                        let effective_ttl = entry.ttl.or_else(|| {
                            ROUTER_CONFIG.with(|c| {
                                c.borrow()
                                    .cache_config
                                    .effective_ttl(NOT_FOUND_CANONICAL_PATH)
                            })
                        });
                        if let Some(ttl) = effective_ttl {
                            let now_ns = ic_cdk::api::time();
                            let expiry_ns =
                                entry.certified_at.saturating_add(ttl.as_nanos() as u64);
                            now_ns >= expiry_ns
                        } else {
                            false
                        }
                    })
                });

                match canonical_state {
                    Some(true) => {
                        // Cached but TTL-expired — upgrade to regenerate.
                        debug_log!("upgrading not-found (TTL expired for canonical path)");
                        return HttpResponse::builder().with_upgrade(true).build();
                    }
                    Some(false) => {
                        // Cached and valid — serve from AssetRouter using the
                        // original request. The /__not_found asset is registered
                        // as a fallback for scope "/", so serve_asset() will
                        // match any path that has no exact asset and produce a
                        // correctly certified response for the original URL.
                        return ASSET_ROUTER.with_borrow(|asset_router| {
                            let cert = match data_certificate() {
                                Some(c) => c,
                                None => {
                                    debug_log!("upgrading not-found (no data certificate)");
                                    return HttpResponse::builder().with_upgrade(true).build();
                                }
                            };
                            if let Ok(response) = asset_router.serve_asset(&cert, &req) {
                                debug_log!("serving cached not-found for {}", path);
                                response
                            } else {
                                debug_log!(
                                    "upgrading not-found (serve_asset failed for canonical path)"
                                );
                                HttpResponse::builder().with_upgrade(true).build()
                            }
                        });
                    }
                    None => {
                        // Not in dynamic cache. Try serving a static asset
                        // for the original path before triggering the update.
                        let maybe_asset = ASSET_ROUTER.with_borrow(|asset_router| {
                            let cert = data_certificate()?;
                            asset_router.serve_asset(&cert, &req).ok()
                        });
                        if let Some(response) = maybe_asset {
                            debug_log!("serving static asset for {}", path);
                            return response;
                        }

                        // No cached response at all — upgrade to run the
                        // not-found handler.
                        debug_log!("upgrading not-found (no cached entry for {})", path);
                        return HttpResponse::builder().with_upgrade(true).build();
                    }
                }
            }

            // Non-certified mode: execute the not-found handler directly
            // without upgrade, same as any other non-certified response.
            if let Some(response) = root_route_node.execute_not_found_with_middleware(&path, req) {
                response
            } else {
                HttpResponse::not_found(
                    b"Not Found",
                    vec![("Content-Type".into(), "text/plain".into())],
                )
                .build()
            }
        }
    }
}

/// Certify a dynamically generated response and store it for future query-path
/// serving.
///
/// The response body is stored in the `AssetRouter` via `certify_assets`,
/// which lets the query path use `serve_asset()`. All responses — including
/// not-found handler output — go through this single path. The not-found
/// handler's response is certified at the canonical `/__not_found` path so
/// that only one cache entry exists for all 404s.
fn certify_dynamic_response(response: HttpResponse<'static>, path: &str) -> HttpResponse<'static> {
    certify_dynamic_response_inner(response, path, vec![])
}

/// Certify a dynamic response with optional fallback configuration.
///
/// When `fallback_for` is non-empty, the asset is registered as a fallback
/// for the given scopes. This is used by the not-found handler to certify
/// a single `/__not_found` asset that serves as a fallback for all paths.
fn certify_dynamic_response_inner(
    response: HttpResponse<'static>,
    path: &str,
    fallback_for: Vec<AssetFallbackConfig>,
) -> HttpResponse<'static> {
    let content_type = extract_content_type(&response);
    let effective_ttl = ROUTER_CONFIG.with(|c| c.borrow().cache_config.effective_ttl(path));

    let asset = Asset::new(path.to_string(), response.body().to_vec());
    let dynamic_cache_control =
        ROUTER_CONFIG.with(|c| c.borrow().cache_control.dynamic_assets.clone());
    let asset_config = IcAssetConfig::File {
        path: path.to_string(),
        content_type: Some(content_type),
        headers: get_asset_headers(vec![("cache-control".to_string(), dynamic_cache_control)]),
        fallback_for,
        aliased_by: vec![],
        encodings: vec![],
    };

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        if let Err(err) = asset_router.certify_assets(vec![asset], vec![asset_config]) {
            ic_cdk::trap(format!("Failed to certify dynamic asset: {err}"));
        }
        certified_data_set(asset_router.root_hash());
    });

    DYNAMIC_CACHE.with(|dc| {
        dc.borrow_mut().insert(
            path.to_string(),
            CachedDynamicAsset {
                certified_at: ic_cdk::api::time(),
                ttl: effective_ttl,
            },
        );
    });

    response
}

/// Handle an HTTP update-path request.
///
/// This is the IC `http_request_update` entry point. It runs the matched
/// route handler (through the middleware chain), certifies the response in
/// the asset router, and caches it for future query-path serving.
///
/// If a [`HandlerResultFn`](router::HandlerResultFn) is registered for the
/// route, it is called first to check for [`HandlerResult::NotModified`].
/// A `NotModified` result preserves the existing cached response and resets
/// the TTL timer (if TTL-based caching is active).
pub fn http_request_update(req: HttpRequest, root_route_node: &RouteNode) -> HttpResponse<'static> {
    debug_log!("http_request_update: {:?}", req.url());

    let path = match req.get_path() {
        Ok(p) => p,
        Err(_) => return error_response(400, "Bad Request: malformed URL"),
    };

    let method = req.method().clone();

    match root_route_node.resolve(&path, &method) {
        RouteResult::Found(handler, params, result_handler) => {
            // If a HandlerResultFn is registered, call it first to check for
            // NotModified. This avoids running the full middleware + certification
            // pipeline when the handler signals that the content hasn't changed.
            if let Some(result_fn) = result_handler {
                let result = result_fn(req.clone(), params.clone());
                match result {
                    router::HandlerResult::NotModified => {
                        debug_log!("handler returned NotModified for {}", path);

                        // Reset the certified_at timestamp if TTL-based caching
                        // is active. The content was confirmed fresh, so the TTL
                        // timer should restart.
                        DYNAMIC_CACHE.with(|dc| {
                            let mut cache = dc.borrow_mut();
                            if let Some(entry) = cache.get_mut(&path) {
                                if entry.ttl.is_some() {
                                    entry.certified_at = ic_cdk::api::time();
                                }
                            }
                        });

                        // Serve the existing cached response from the asset router.
                        return ASSET_ROUTER.with_borrow(|asset_router| {
                            let cert = match data_certificate() {
                                Some(c) => c,
                                None => {
                                    // In an update call, data_certificate() is unavailable.
                                    // Return the cached body without certification headers.
                                    // The next query call will attach the valid proof.
                                    return match asset_router.serve_asset(&[], &req) {
                                        Ok(resp) => resp,
                                        Err(_) => error_response(
                                            500,
                                            "Internal Server Error: NotModified but no cached asset found",
                                        ),
                                    };
                                }
                            };
                            match asset_router.serve_asset(&cert, &req) {
                                Ok(resp) => resp,
                                Err(_) => error_response(
                                    500,
                                    "Internal Server Error: NotModified but no cached asset found",
                                ),
                            }
                        });
                    }
                    router::HandlerResult::Response(response) => {
                        // Handler produced a new response — proceed with the
                        // standard certification pipeline below. We use the
                        // response from the HandlerResultFn directly (middleware
                        // has already been bypassed for result handlers).
                        return certify_dynamic_response(response, &path);
                    }
                }
            }

            // Standard path: call handler through middleware chain, then certify.
            let response = root_route_node.execute_with_middleware(&path, handler, req, params);
            certify_dynamic_response(response, &path)
        }
        RouteResult::MethodNotAllowed(allowed) => method_not_allowed(&allowed),
        RouteResult::NotFound => {
            // Check if the canonical 404 entry already has a valid cached
            // response. If so, skip re-execution and serve directly.
            let cached_valid = DYNAMIC_CACHE.with(|dc| {
                let cache = dc.borrow();
                if let Some(entry) = cache.get(NOT_FOUND_CANONICAL_PATH) {
                    let effective_ttl = entry.ttl.or_else(|| {
                        ROUTER_CONFIG.with(|c| {
                            c.borrow()
                                .cache_config
                                .effective_ttl(NOT_FOUND_CANONICAL_PATH)
                        })
                    });
                    let expired = if let Some(ttl) = effective_ttl {
                        let now_ns = ic_cdk::api::time();
                        let expiry_ns = entry.certified_at.saturating_add(ttl.as_nanos() as u64);
                        now_ns >= expiry_ns
                    } else {
                        false
                    };
                    !expired
                } else {
                    false
                }
            });

            if cached_valid {
                // Already certified and not expired — serve from asset router.
                debug_log!("not-found canonical entry still valid, serving from cache");
                return ASSET_ROUTER.with_borrow(|asset_router| {
                    let canonical_req = HttpRequest::get(NOT_FOUND_CANONICAL_PATH.to_string()).build();
                    match asset_router.serve_asset(&[], &canonical_req) {
                        Ok(resp) => resp,
                        Err(_) => error_response(
                            500,
                            "Internal Server Error: cached not-found entry missing from asset router",
                        ),
                    }
                });
            }

            // Execute the not-found handler and certify at the canonical path.
            let response = if let Some(response) =
                root_route_node.execute_not_found_with_middleware(&path, req)
            {
                response
            } else {
                HttpResponse::not_found(
                    b"Not Found",
                    vec![("Content-Type".into(), "text/plain".into())],
                )
                .build()
            };
            certify_dynamic_response_inner(
                response,
                NOT_FOUND_CANONICAL_PATH,
                vec![AssetFallbackConfig {
                    scope: "/".to_string(),
                    status_code: Some(StatusCode::NOT_FOUND),
                }],
            )
        }
    }
}

// Test coverage audit (Session 7, Spec 5.5):
//
// Covered:
//   - Malformed URL → 400 response (both http_request and http_request_update)
//   - Handler without content-type doesn't panic
//   - extract_content_type: JSON, HTML, missing (fallback to octet-stream), case-insensitive
//
// No significant gaps for unit-testable code. IC runtime-dependent paths
// (certification, asset serving, TTL upgrade, NotModified flow) require PocketIC
// E2E tests (spec 5.7).
#[cfg(test)]
mod tests {
    use super::*;
    use ic_http_certification::Method;
    use router::{NodeType, RouteNode, RouteParams};

    fn noop_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_body(b"ok" as &[u8])
            .build()
    }

    fn setup_router() -> RouteNode {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/", Method::GET, noop_handler);
        root.insert("/*", Method::GET, noop_handler);
        root
    }

    // ---- 1.3.5: malformed URL returns 400 (not a trap) ----

    #[test]
    fn http_request_malformed_url_returns_400() {
        let root = setup_router();
        // Construct a request with a URL that will fail `get_path()` parsing.
        // A bare NUL byte in the URL makes the URI parser fail.
        let req = HttpRequest::builder()
            .with_method(Method::GET)
            .with_url("http://[::bad")
            .build();
        let opts = HttpRequestOptions { certify: false };
        let response = http_request(req, &root, opts);
        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
        assert!(std::str::from_utf8(response.body())
            .unwrap()
            .contains("malformed URL"));
    }

    #[test]
    fn http_request_update_malformed_url_returns_400() {
        let root = setup_router();
        let req = HttpRequest::builder()
            .with_method(Method::GET)
            .with_url("http://[::bad")
            .build();
        let response = http_request_update(req, &root);
        assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
        assert!(std::str::from_utf8(response.body())
            .unwrap()
            .contains("malformed URL"));
    }

    // ---- 1.3.6: missing content-type in handler response doesn't trap ----

    fn handler_no_content_type(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        // Response with no content-type header — should not cause the library to trap.
        HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_body(b"no content-type" as &[u8])
            .build()
    }

    #[test]
    fn handler_without_content_type_does_not_trap() {
        // This test verifies that a handler returning a response without a
        // content-type header does not cause a panic. The http_request_update
        // function calls IC runtime APIs (certify_assets, certified_data_set)
        // that are unavailable in unit tests, so we test the handler directly
        // and verify the router dispatch path up to the handler call.
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/no-ct", Method::GET, handler_no_content_type);

        // Verify the route matches and the handler runs without panic.
        let req = HttpRequest::builder()
            .with_method(Method::GET)
            .with_url("/no-ct")
            .build();
        match root.resolve("/no-ct", &Method::GET) {
            RouteResult::Found(handler, params, _) => {
                let response = handler(req, params);
                assert_eq!(response.status_code(), StatusCode::OK);
                assert_eq!(response.body(), b"no content-type");
                // No content-type header present — and no panic occurred.
                assert!(response
                    .headers()
                    .iter()
                    .all(|(name, _): &(String, String)| name.to_lowercase() != "content-type"));
            }
            _ => panic!("expected Found for GET /no-ct"),
        }
    }

    // ---- 1.2: handler-controlled response metadata ----

    #[test]
    fn extract_content_type_json() {
        let response = HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )])
            .with_body(b"{}" as &[u8])
            .build();
        assert_eq!(extract_content_type(&response), "application/json");
    }

    #[test]
    fn extract_content_type_html() {
        let response = HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![("Content-Type".to_string(), "text/html".to_string())])
            .with_body(b"<h1>hi</h1>" as &[u8])
            .build();
        assert_eq!(extract_content_type(&response), "text/html");
    }

    #[test]
    fn extract_content_type_missing_falls_back() {
        let response = HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_body(b"raw bytes" as &[u8])
            .build();
        assert_eq!(extract_content_type(&response), "application/octet-stream");
    }

    #[test]
    fn extract_content_type_case_insensitive() {
        let response = HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![("CONTENT-TYPE".to_string(), "text/plain".to_string())])
            .with_body(b"hello" as &[u8])
            .build();
        assert_eq!(extract_content_type(&response), "text/plain");
    }
}

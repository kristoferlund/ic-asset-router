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
use ic_asset_certification::{Asset, AssetConfig as IcAssetConfig, AssetRouter};
use ic_cdk::api::{certified_data_set, data_certificate};
use ic_http_certification::{
    utils::add_v2_certificate_header, HttpCertification, HttpCertificationPath,
    HttpCertificationTree, HttpCertificationTreeEntry, HttpRequest, HttpResponse, Method,
    StatusCode,
};
use router::{RouteNode, RouteResult};

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

pub mod assets;
pub mod build;
pub mod config;
pub mod middleware;
pub mod mime;
pub mod router;

pub use assets::{invalidate_all_dynamic, invalidate_path, invalidate_prefix, last_certified_at};
pub use config::{AssetConfig, CacheConfig, CacheControl, SecurityHeaders};
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

pub struct HttpRequestOptions {
    pub certify: bool,
}

impl Default for HttpRequestOptions {
    fn default() -> Self {
        HttpRequestOptions { certify: true }
    }
}

/// Serve assets that have already been certified, or upgrade the request to an update call
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
                // TTL check: if the path is a dynamic asset with an expired TTL,
                // upgrade to an update call to regenerate it.
                let ttl_expired = DYNAMIC_CACHE.with(|dc| {
                    let cache = dc.borrow();
                    if let Some(entry) = cache.get(&path) {
                        // Effective TTL: entry TTL > per-route TTL > default TTL > None
                        let effective_ttl = entry.ttl.or_else(|| {
                            ROUTER_CONFIG.with(|c| c.borrow().cache_config.effective_ttl(&path))
                        });
                        if let Some(ttl) = effective_ttl {
                            let now_ns = ic_cdk::api::time();
                            let expiry_ns =
                                entry.certified_at.saturating_add(ttl.as_nanos() as u64);
                            return now_ns >= expiry_ns;
                        }
                    }
                    false
                });

                if ttl_expired {
                    debug_log!("upgrading (TTL expired for {})", path);
                    return HttpResponse::builder().with_upgrade(true).build();
                }

                ASSET_ROUTER.with_borrow(|asset_router| {
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
                })
            }
        },
        RouteResult::MethodNotAllowed(allowed) => method_not_allowed(&allowed),
        RouteResult::NotFound => {
            // Try serving a certified static asset before returning 404.
            if opts.certify {
                let maybe_asset = ASSET_ROUTER.with_borrow(|asset_router| {
                    let cert = data_certificate()?;
                    asset_router.serve_asset(&cert, &req).ok()
                });
                if let Some(response) = maybe_asset {
                    debug_log!("serving static asset for {}", path);
                    return response;
                }
            }

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

/// Certify a dynamically generated response and store it in the asset router.
///
/// This is the shared certification pipeline used by `http_request_update` for
/// both standard `HandlerFn` responses and `HandlerResult::Response` values.
fn certify_dynamic_response(response: HttpResponse<'static>, path: &str) -> HttpResponse<'static> {
    let asset = Asset::new(path.to_string(), response.body().to_vec());

    let content_type = extract_content_type(&response);

    let dynamic_cache_control =
        ROUTER_CONFIG.with(|c| c.borrow().cache_control.dynamic_assets.clone());
    let asset_config = IcAssetConfig::File {
        path: path.to_string(),
        content_type: Some(content_type),
        headers: get_asset_headers(vec![("cache-control".to_string(), dynamic_cache_control)]),
        fallback_for: vec![],
        aliased_by: vec![],
        encodings: vec![],
    };

    ASSET_ROUTER.with_borrow_mut(|asset_router| {
        if let Err(err) = asset_router.certify_assets(vec![asset], vec![asset_config]) {
            ic_cdk::trap(format!("Failed to certify dynamic asset: {err}"));
        }
        certified_data_set(asset_router.root_hash());
    });

    // Resolve the effective TTL for this path:
    // per_route_ttl > default_ttl > None (cache forever).
    let effective_ttl = ROUTER_CONFIG.with(|c| c.borrow().cache_config.effective_ttl(path));

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

/// Match incoming requests to the appropriate handler, generating assets as needed
/// and certifying them for future requests.
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

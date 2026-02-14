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

use std::{borrow::Cow, cell::RefCell, rc::Rc};

use assets::get_asset_headers;
use ic_asset_certification::{Asset, AssetConfig as IcAssetConfig, AssetRouter};
use ic_cdk::api::{certified_data_set, data_certificate};
use ic_http_certification::{
    utils::add_v2_certificate_header, HttpCertification, HttpCertificationPath,
    HttpCertificationTree, HttpCertificationTreeEntry, HttpRequest, HttpResponse, StatusCode,
};
use router::RouteNode;

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
pub mod mime;
pub mod router;

pub use config::{AssetConfig, CacheControl, SecurityHeaders};

thread_local! {
    static HTTP_TREE: Rc<RefCell<HttpCertificationTree>> = Default::default();
    static ASSET_ROUTER: RefCell<AssetRouter<'static>> = RefCell::new(AssetRouter::with_tree(HTTP_TREE.with(|tree| tree.clone())));
    static ROUTER_CONFIG: RefCell<AssetConfig> = RefCell::new(AssetConfig::default());
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
    match root_route_node.match_path(&path) {
        Some((handler, params)) => match opts.certify {
            false => {
                debug_log!("Serving {} without certification", path);
                let mut response = handler(req, params);

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
            true => ASSET_ROUTER.with_borrow(|asset_router| {
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
        },
        None => HttpResponse::not_found(
            b"Not Found",
            vec![("Content-Type".into(), "text/plain".into())],
        )
        .build(),
    }
}

/// Match incoming requests to the appropriate handler, generating assets as needed
/// and certifying them for future requests.
pub fn http_request_update(req: HttpRequest, root_route_node: &RouteNode) -> HttpResponse<'static> {
    debug_log!("http_request_update: {:?}", req.url());

    let path = match req.get_path() {
        Ok(p) => p,
        Err(_) => return error_response(400, "Bad Request: malformed URL"),
    };
    match root_route_node.match_path(&path) {
        Some((handler, params)) => {
            let response = handler(req, params);

            let asset = Asset::new(path.clone(), response.body().to_vec());

            let content_type = extract_content_type(&response);

            let dynamic_cache_control =
                ROUTER_CONFIG.with(|c| c.borrow().cache_control.dynamic_assets.clone());
            let asset_config = IcAssetConfig::File {
                path: path.to_string(),
                content_type: Some(content_type),
                headers: get_asset_headers(vec![(
                    "cache-control".to_string(),
                    dynamic_cache_control,
                )]),
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

            response
        }
        None => HttpResponse::not_found(
            b"Not Found",
            vec![("Content-Type".into(), "text/plain".into())],
        )
        .build(),
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
        root.insert("/", noop_handler);
        root.insert("/*", noop_handler);
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
        root.insert("/no-ct", handler_no_content_type);

        // Verify the route matches and the handler runs without panic.
        let req = HttpRequest::builder()
            .with_method(Method::GET)
            .with_url("/no-ct")
            .build();
        let (handler, params) = root.match_path("/no-ct").unwrap();
        let response = handler(req, params);
        assert_eq!(response.status_code(), StatusCode::OK);
        assert_eq!(response.body(), b"no content-type");
        // No content-type header present — and no panic occurred.
        assert!(response
            .headers()
            .iter()
            .all(|(name, _)| name.to_lowercase() != "content-type"));
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

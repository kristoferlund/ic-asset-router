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

use std::{cell::RefCell, rc::Rc};

use assets::{get_asset_headers, NO_CACHE_ASSET_CACHE_CONTROL};
use ic_asset_certification::{Asset, AssetConfig, AssetRouter};
use ic_cdk::api::{certified_data_set, data_certificate};
use ic_http_certification::{
    utils::add_v2_certificate_header, HttpCertification, HttpCertificationPath,
    HttpCertificationTree, HttpCertificationTreeEntry, HttpRequest, HttpResponse,
};
use router::RouteNode;

pub mod assets;
pub mod build;
pub mod mime;
pub mod router;

thread_local! {
    static HTTP_TREE: Rc<RefCell<HttpCertificationTree>> = Default::default();
    static ASSET_ROUTER: RefCell<AssetRouter<'static>> = RefCell::new(AssetRouter::with_tree(HTTP_TREE.with(|tree| tree.clone())));
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

    let path = req.get_path().unwrap();
    match root_route_node.match_path(&path) {
        Some((handler, params)) => match opts.certify {
            false => {
                debug_log!("Serving {} without certification", path);
                let mut response = handler(req, params);

                HTTP_TREE.with(|tree| {
                    let tree = tree.borrow();

                    // Use exact "/" to match the tree entry created during init
                    let tree_path = HttpCertificationPath::exact("/");
                    let certification = HttpCertification::skip();
                    let tree_entry = HttpCertificationTreeEntry::new(&tree_path, certification);

                    add_v2_certificate_header(
                        &data_certificate().expect("No data certificate available"),
                        &mut response,
                        &tree
                            .witness(&tree_entry, "/")
                            .expect("Failed to create witness for /"),
                        &tree_path.to_expr_path(),
                    );

                    response
                })
            }
            true => ASSET_ROUTER.with_borrow(|asset_router| {
                if let Ok(response) = asset_router.serve_asset(&data_certificate().unwrap(), &req) {
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

    let path = req.get_path().unwrap();
    match root_route_node.match_path(&path) {
        Some((handler, params)) => {
            let response = handler(req, params);

            let asset = Asset::new(path.clone(), response.body().to_vec());

            // TODO: handlers should be able to specify asset settings when they generate assets
            let asset_config = AssetConfig::File {
                path: path.to_string(),
                content_type: Some("text/html".to_string()),
                headers: get_asset_headers(vec![(
                    "cache-control".to_string(),
                    NO_CACHE_ASSET_CACHE_CONTROL.to_string(),
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

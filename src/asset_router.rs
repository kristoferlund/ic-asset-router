/// Asset router with per-asset certification modes.
///
/// This module provides the [`AssetRouter`] — a unified store for certified
/// assets (both static and dynamic). It replaces the external
/// `ic-asset-certification::AssetRouter` with a custom implementation built
/// directly on `ic-http-certification` primitives.
///
/// # Capabilities
///
/// - **Per-asset certification modes** — each asset can independently use
///   [`Skip`](crate::CertificationMode::Skip),
///   [`ResponseOnly`](crate::CertificationMode::ResponseOnly), or
///   [`Full`](crate::CertificationMode::Full) certification.
/// - **Encoding negotiation** — Brotli, Gzip, and Identity variants are
///   stored per-asset and the best encoding is selected based on the
///   client's `Accept-Encoding` header.
/// - **Fallback/scope matching** — assets can be registered as fallbacks
///   for a scope (e.g., SPA index for `/`). Longest-prefix match wins.
/// - **Path aliases** — multiple paths can map to the same asset
///   (e.g., `/` and `/index.html`).
/// - **TTL-based expiry** — dynamic assets can have an optional TTL for
///   automatic cache invalidation.
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use ic_http_certification::{
    cel::DefaultRequestCertification, DefaultCelBuilder, DefaultResponseCertification,
    DefaultResponseOnlyCelExpression, HeaderField, HttpCertification, HttpCertificationPath,
    HttpCertificationTree, HttpCertificationTreeEntry, HttpRequest, HttpResponse, StatusCode,
    CERTIFICATE_EXPRESSION_HEADER_NAME,
};

use crate::certification::{CertificationMode, ResponseOnlyConfig};
use crate::mime::get_mime_type;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Content encoding variants supported by the asset router.
///
/// When serving an asset, the router selects the best encoding based on
/// the client's `Accept-Encoding` header, preferring Brotli over Gzip
/// over Identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssetEncoding {
    /// No encoding (identity). Always available for every asset.
    Identity,
    /// Gzip compression (`Content-Encoding: gzip`).
    Gzip,
    /// Brotli compression (`Content-Encoding: br`). Preferred when available.
    Brotli,
}

impl AssetEncoding {
    /// Content-Encoding header value for this encoding.
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetEncoding::Identity => "identity",
            AssetEncoding::Gzip => "gzip",
            AssetEncoding::Brotli => "br",
        }
    }
}

/// A certified asset stored in the [`AssetRouter`].
///
/// Contains the asset body, compressed variants, response metadata,
/// certification state, and tree entry for witness generation. Each
/// `CertifiedAsset` is stored at a canonical path in the router; aliases
/// and fallback scopes reference back to the canonical entry.
///
/// This struct intentionally does **not** derive `Clone` because
/// [`HttpCertificationTreeEntry`] references the certification tree and
/// should not be duplicated.
pub struct CertifiedAsset {
    /// Raw content (for Identity encoding).
    pub content: Vec<u8>,

    /// Encoded variants (Gzip, Brotli).
    pub encodings: HashMap<AssetEncoding, Vec<u8>>,

    /// MIME type (e.g., "text/html").
    pub content_type: String,

    /// HTTP status code for the response (e.g., 200, 404).
    pub status_code: StatusCode,

    /// Additional headers to include in response.
    pub headers: Vec<HeaderField>,

    /// Certification mode for this asset.
    pub certification_mode: CertificationMode,

    /// Pre-computed CEL expression string.
    pub cel_expression: String,

    /// Tree entry for generating witnesses.
    pub tree_entry: HttpCertificationTreeEntry<'static>,

    /// Whether this asset can serve as a fallback for paths in its scope.
    pub fallback_scope: Option<String>,

    /// Additional paths that alias to this asset.
    pub aliases: Vec<String>,

    /// Timestamp (nanoseconds) when the asset was certified.
    /// Enables TTL-based cache invalidation.
    pub certified_at: u64,

    /// Optional TTL for cache invalidation.
    /// `None` means the asset never expires (static assets).
    /// `Some(duration)` enables TTL-based invalidation (dynamic assets).
    pub ttl: Option<Duration>,

    /// Whether this asset was dynamically generated (via `http_request_update`).
    ///
    /// Dynamic assets may or may not have a TTL. Without TTL they persist
    /// until explicitly invalidated; with TTL they expire automatically.
    /// This flag is separate from `ttl` to support the unified router's
    /// need to track all dynamically-generated assets regardless of TTL.
    pub dynamic: bool,
}

impl CertifiedAsset {
    /// Returns true if this asset was dynamically generated.
    ///
    /// A dynamic asset is one that was certified via `certify_dynamic_response_with_ttl`
    /// (i.e., from `http_request_update`), as opposed to a static asset
    /// certified during `init`/`post_upgrade`.
    pub fn is_dynamic(&self) -> bool {
        self.dynamic
    }

    /// Check if the asset has expired based on current time (nanoseconds).
    pub fn is_expired(&self, now_ns: u64) -> bool {
        match self.ttl {
            None => false,
            Some(ttl) => {
                let expiry_ns = self.certified_at.saturating_add(ttl.as_nanos() as u64);
                now_ns >= expiry_ns
            }
        }
    }
}

/// Configuration passed to [`AssetRouter::certify_asset`] or
/// [`AssetRouter::certify_dynamic_asset`] when registering an asset.
///
/// Controls the certification mode, content type, response headers,
/// pre-compressed encoding variants, fallback scope, path aliases,
/// certification timestamp, and TTL.
///
/// # Default
///
/// The default configuration uses [`CertificationMode::response_only()`],
/// auto-detects the content type from the file extension, and has no TTL
/// (static asset that never expires).
pub struct AssetCertificationConfig {
    /// Certification mode (determines CEL expression).
    pub mode: CertificationMode,

    /// MIME type. Auto-detected from path if not provided.
    pub content_type: Option<String>,

    /// HTTP status code for the response. Defaults to 200 OK.
    pub status_code: StatusCode,

    /// Additional headers.
    pub headers: Vec<HeaderField>,

    /// Available encodings (content should be pre-compressed).
    pub encodings: Vec<(AssetEncoding, Vec<u8>)>,

    /// Fallback scope (e.g., "/" for SPA fallback).
    pub fallback_for: Option<String>,

    /// Path aliases.
    pub aliases: Vec<String>,

    /// Timestamp (nanoseconds) when the asset was certified.
    /// Use `ic_cdk::api::time()` for the current time.
    pub certified_at: u64,

    /// Optional TTL for cache invalidation.
    /// `None` means the asset never expires (static assets).
    /// `Some(duration)` enables TTL-based invalidation (dynamic assets).
    pub ttl: Option<Duration>,

    /// Whether this asset was dynamically generated.
    pub dynamic: bool,
}

impl Default for AssetCertificationConfig {
    fn default() -> Self {
        Self {
            mode: CertificationMode::response_only(),
            content_type: None,
            status_code: StatusCode::OK,
            headers: vec![],
            encodings: vec![],
            fallback_for: None,
            aliases: vec![],
            certified_at: 0,
            ttl: None,
            dynamic: false,
        }
    }
}

/// Errors returned by [`AssetRouter`] certification and serving methods.
#[derive(Debug)]
pub enum AssetRouterError {
    /// The certification process itself failed (upstream library error).
    CertificationFailed(String),
    /// `certify_asset()` was called with `Full` mode. Use
    /// `certify_dynamic_asset()` instead, which accepts the request.
    FullModeRequiresRequest,
    /// Asset not found at the given path.
    AssetNotFound(String),
}

impl std::fmt::Display for AssetRouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetRouterError::CertificationFailed(msg) => {
                write!(f, "Failed to create certification: {}", msg)
            }
            AssetRouterError::FullModeRequiresRequest => {
                write!(
                    f,
                    "Full certification mode requires a request; use certify_dynamic_asset()"
                )
            }
            AssetRouterError::AssetNotFound(path) => {
                write!(f, "Asset not found: {}", path)
            }
        }
    }
}

impl std::error::Error for AssetRouterError {}

// ---------------------------------------------------------------------------
// AssetRouter
// ---------------------------------------------------------------------------

/// Unified asset store with per-asset certification modes.
///
/// Stores both static assets (embedded at compile time via
/// [`certify_assets`](crate::assets::certify_assets)) and dynamic assets
/// (generated at runtime via [`certify_dynamic_asset`](Self::certify_dynamic_asset)).
/// All assets share a single [`HttpCertificationTree`] so the canister
/// exposes one consistent root hash.
///
/// # Lookup Order
///
/// When [`serve_asset`](Self::serve_asset) is called:
///
/// 1. **Exact match** — canonical path in the asset map.
/// 2. **Alias resolution** — alias path mapped to a canonical path.
/// 3. **Fallback match** — longest-prefix fallback scope that covers
///    the request path (e.g., a SPA index registered for `/`).
///
/// # Thread Safety
///
/// `AssetRouter` is **not** `Send` or `Sync` because it holds an
/// `Rc<RefCell<HttpCertificationTree>>`. On the IC this is fine — all
/// canister code runs single-threaded. Store it in a `thread_local!`.
pub struct AssetRouter {
    /// Certified assets by canonical path.
    assets: HashMap<String, CertifiedAsset>,

    /// Alias path -> canonical path.
    aliases: HashMap<String, String>,

    /// Shared certification tree.
    tree: Rc<RefCell<HttpCertificationTree>>,

    /// Fallback assets by scope, sorted by scope length descending
    /// so that longest-prefix match wins.
    fallbacks: Vec<(String, String)>,
}

impl AssetRouter {
    /// Create a new router with the given certification tree.
    pub fn with_tree(tree: Rc<RefCell<HttpCertificationTree>>) -> Self {
        Self {
            assets: HashMap::new(),
            aliases: HashMap::new(),
            tree,
            fallbacks: Vec::new(),
        }
    }

    /// Get the root hash of the certification tree.
    pub fn root_hash(&self) -> [u8; 32] {
        self.tree.borrow().root_hash()
    }

    /// Check if an asset exists at the given path (canonical or alias).
    pub fn contains_asset(&self, path: &str) -> bool {
        let canonical = self.aliases.get(path).map(|s| s.as_str()).unwrap_or(path);
        self.assets.contains_key(canonical)
    }

    /// Get a reference to a certified asset by path (canonical or alias).
    pub fn get_asset(&self, path: &str) -> Option<&CertifiedAsset> {
        let canonical = self.aliases.get(path).map(|s| s.as_str()).unwrap_or(path);
        self.assets.get(canonical)
    }

    /// Get a mutable reference to a certified asset by path.
    pub fn get_asset_mut(&mut self, path: &str) -> Option<&mut CertifiedAsset> {
        let canonical = self
            .aliases
            .get(path)
            .map(|s| s.as_str())
            .unwrap_or(path)
            .to_string();
        self.assets.get_mut(&canonical)
    }

    /// Return all canonical paths of dynamic assets.
    pub fn dynamic_paths(&self) -> Vec<String> {
        self.assets
            .iter()
            .filter(|(_, asset)| asset.is_dynamic())
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Return all canonical paths of dynamic assets whose path starts with `prefix`.
    pub fn dynamic_paths_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.assets
            .iter()
            .filter(|(path, asset)| asset.is_dynamic() && path.starts_with(prefix))
            .map(|(path, _)| path.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CEL expression helpers
// ---------------------------------------------------------------------------

/// Build the CEL expression string from a `CertificationMode`.
///
/// Returns the string representation used in the `ic-certificateexpression`
/// header. The typed expression structs are constructed inline where needed
/// (in `create_certification`) because they borrow from local data.
fn build_cel_expression_string(mode: &CertificationMode) -> String {
    match mode {
        CertificationMode::Skip => DefaultCelBuilder::skip_certification().to_string(),
        CertificationMode::ResponseOnly(config) => {
            let response_cert = build_response_certification(config);
            DefaultCelBuilder::response_only_certification()
                .with_response_certification(response_cert)
                .build()
                .to_string()
        }
        CertificationMode::Full(config) => {
            let response_cert = build_response_certification(&config.response);
            let req_refs: Vec<&str> = config.request_headers.iter().map(|s| s.as_str()).collect();
            let qp_refs: Vec<&str> = config.query_params.iter().map(|s| s.as_str()).collect();

            let mut builder = DefaultCelBuilder::full_certification();
            if !req_refs.is_empty() {
                builder = builder.with_request_headers(req_refs);
            }
            if !qp_refs.is_empty() {
                builder = builder.with_request_query_parameters(qp_refs);
            }
            builder
                .with_response_certification(response_cert)
                .build()
                .to_string()
        }
    }
}

/// Build a `DefaultResponseCertification` from a `ResponseOnlyConfig`.
fn build_response_certification<'a>(
    config: &'a ResponseOnlyConfig,
) -> DefaultResponseCertification<'a> {
    if config.include_headers == vec!["*"] {
        let exclude_refs: Vec<&str> = config.exclude_headers.iter().map(|s| s.as_str()).collect();
        DefaultResponseCertification::response_header_exclusions(exclude_refs)
    } else {
        let include_refs: Vec<&str> = config.include_headers.iter().map(|s| s.as_str()).collect();
        DefaultResponseCertification::certified_response_headers(include_refs)
    }
}

/// Create an `HttpCertification` for the Skip or ResponseOnly modes.
///
/// `Full` mode is not handled here — use `create_full_certification` instead.
fn create_certification(
    mode: &CertificationMode,
    response: &HttpResponse<'_>,
) -> Result<HttpCertification, AssetRouterError> {
    match mode {
        CertificationMode::Skip => Ok(HttpCertification::skip()),
        CertificationMode::ResponseOnly(config) => {
            let response_cert = build_response_certification(config);
            let expr = DefaultResponseOnlyCelExpression {
                response: response_cert,
            };
            HttpCertification::response_only(&expr, response, None)
                .map_err(|e| AssetRouterError::CertificationFailed(e.to_string()))
        }
        CertificationMode::Full(_) => Err(AssetRouterError::FullModeRequiresRequest),
    }
}

/// Create an `HttpCertification` for any mode including Full.
fn create_certification_with_request(
    mode: &CertificationMode,
    request: &HttpRequest,
    response: &HttpResponse<'_>,
) -> Result<HttpCertification, AssetRouterError> {
    match mode {
        CertificationMode::Skip => Ok(HttpCertification::skip()),
        CertificationMode::ResponseOnly(config) => {
            let response_cert = build_response_certification(config);
            let expr = DefaultResponseOnlyCelExpression {
                response: response_cert,
            };
            HttpCertification::response_only(&expr, response, None)
                .map_err(|e| AssetRouterError::CertificationFailed(e.to_string()))
        }
        CertificationMode::Full(config) => {
            let response_cert = build_response_certification(&config.response);
            let req_refs: Vec<&str> = config.request_headers.iter().map(|s| s.as_str()).collect();
            let qp_refs: Vec<&str> = config.query_params.iter().map(|s| s.as_str()).collect();
            let expr = ic_http_certification::DefaultFullCelExpression {
                request: DefaultRequestCertification::new(req_refs, qp_refs),
                response: response_cert,
            };
            HttpCertification::full(&expr, request, response, None)
                .map_err(|e| AssetRouterError::CertificationFailed(e.to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// Core implementation
// ---------------------------------------------------------------------------

impl AssetRouter {
    /// Shared certification logic for both static and dynamic assets.
    ///
    /// - `response_for_cert`: when `Some`, the caller provides the response
    ///   (dynamic path — the CEL header is added to a copy). When `None`, the
    ///   method builds one from `body` + config headers (static path).
    /// - `request`: when `Some`, uses request-aware certification (Full mode).
    ///   When `None`, uses response-only or skip certification.
    fn certify_inner(
        &mut self,
        path: &str,
        body: Vec<u8>,
        response_for_cert: Option<&HttpResponse<'static>>,
        request: Option<&HttpRequest>,
        config: AssetCertificationConfig,
    ) -> Result<(), AssetRouterError> {
        let content_type = config
            .content_type
            .unwrap_or_else(|| get_mime_type(path).to_string());

        let cel_str = build_cel_expression_string(&config.mode);

        // Build or augment the response used for certification.
        let cert_response = match response_for_cert {
            None => {
                // Static path: build from body + config headers.
                let mut all_headers = vec![
                    ("content-type".to_string(), content_type.clone()),
                    (
                        CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
                        cel_str.clone(),
                    ),
                ];
                for (name, value) in &config.headers {
                    all_headers.push((name.clone(), value.clone()));
                }
                HttpResponse::builder()
                    .with_status_code(config.status_code)
                    .with_headers(all_headers)
                    .with_body(body.as_slice())
                    .build()
            }
            Some(resp) => {
                // Dynamic path: add CEL header to a copy of the provided response.
                let mut cert_headers: Vec<HeaderField> = resp.headers().to_vec();
                cert_headers.push((
                    CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
                    cel_str.clone(),
                ));
                HttpResponse::builder()
                    .with_status_code(resp.status_code())
                    .with_headers(cert_headers)
                    .with_body(Cow::<[u8]>::Owned(resp.body().to_vec()))
                    .build()
            }
        };

        // Create certification.
        let certification = match request {
            Some(req) => create_certification_with_request(&config.mode, req, &cert_response)?,
            None => create_certification(&config.mode, &cert_response)?,
        };

        // Create tree entry and insert. We pass an owned String so the
        // `HttpCertificationPath` stores a `Cow::Owned`, making the
        // resulting `HttpCertificationTreeEntry` have `'static` lifetime.
        //
        // For fallback assets, use a wildcard path so the certification is
        // valid for any request URL under the scope. For exact assets, use
        // the exact path.
        let tree_path = if let Some(ref scope) = config.fallback_for {
            HttpCertificationPath::wildcard(scope.to_string())
        } else {
            HttpCertificationPath::exact(path.to_string())
        };
        let tree_entry = HttpCertificationTreeEntry::new(tree_path, certification);

        // Remove stale tree entry if re-certifying an existing path.
        if let Some(old_asset) = self.assets.get(path) {
            self.tree.borrow_mut().delete(&old_asset.tree_entry);
        }

        self.tree.borrow_mut().insert(&tree_entry);

        // Build encodings map.
        let mut encodings = HashMap::new();
        encodings.insert(AssetEncoding::Identity, body.clone());
        for (encoding, encoded_content) in config.encodings {
            encodings.insert(encoding, encoded_content);
        }

        // Store asset.
        let asset = CertifiedAsset {
            content: body,
            encodings,
            content_type,
            status_code: config.status_code,
            headers: config.headers,
            certification_mode: config.mode,
            cel_expression: cel_str,
            tree_entry,
            fallback_scope: config.fallback_for.clone(),
            aliases: config.aliases.clone(),
            certified_at: config.certified_at,
            ttl: config.ttl,
            dynamic: config.dynamic,
        };

        self.assets.insert(path.to_string(), asset);

        // Register fallback if specified (sorted longest-first).
        if let Some(scope) = config.fallback_for {
            self.fallbacks.push((scope, path.to_string()));
            self.fallbacks.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        }

        // Register aliases.
        for alias in config.aliases {
            self.aliases.insert(alias, path.to_string());
        }

        Ok(())
    }

    /// Certify a single asset (static or dynamic).
    ///
    /// For `Skip` and `ResponseOnly` modes, this is the standard entry point.
    /// For `Full` mode, use [`certify_dynamic_asset`](Self::certify_dynamic_asset)
    /// which accepts the request.
    pub fn certify_asset(
        &mut self,
        path: &str,
        content: Vec<u8>,
        config: AssetCertificationConfig,
    ) -> Result<(), AssetRouterError> {
        if matches!(&config.mode, CertificationMode::Full(_)) {
            return Err(AssetRouterError::FullModeRequiresRequest);
        }
        self.certify_inner(path, content, None, None, config)
    }

    /// Certify a dynamic asset with any mode, including `Full`.
    ///
    /// Unlike `certify_asset`, this takes the original request so that
    /// request method, body, headers, and query params can participate
    /// in the certification hash.
    pub fn certify_dynamic_asset(
        &mut self,
        path: &str,
        request: &HttpRequest,
        response: &HttpResponse<'static>,
        config: AssetCertificationConfig,
    ) -> Result<(), AssetRouterError> {
        self.certify_inner(
            path,
            response.body().to_vec(),
            Some(response),
            Some(request),
            config,
        )
    }

    /// Serve an asset for the given request.
    ///
    /// Returns `None` if no matching asset is found.
    /// Returns the response, the witness (hash tree), and the expression path
    /// needed to construct the `ic-certificate` header.
    ///
    /// The caller is responsible for calling `add_v2_certificate_header`
    /// with the returned witness, expression path, and the IC data
    /// certificate to produce the final servable response.
    pub fn serve_asset(
        &self,
        request: &HttpRequest,
    ) -> Option<(
        HttpResponse<'static>,
        ic_certification::HashTree,
        Vec<String>,
    )> {
        let path = request.get_path().ok()?;

        // 1. Try exact match (canonical path).
        if let Some(asset) = self.assets.get(&path) {
            return self.serve_matched_asset(request, &path, asset);
        }

        // 2. Try alias -> canonical resolution.
        if let Some(canonical) = self.aliases.get(&path) {
            if let Some(asset) = self.assets.get(canonical) {
                return self.serve_matched_asset(request, &path, asset);
            }
        }

        // 3. Try fallback (sorted longest-first, so first match wins).
        for (scope, fallback_path) in &self.fallbacks {
            if path.starts_with(scope) {
                if let Some(asset) = self.assets.get(fallback_path) {
                    return self.serve_matched_asset(request, &path, asset);
                }
            }
        }

        None
    }

    fn serve_matched_asset(
        &self,
        request: &HttpRequest,
        request_path: &str,
        asset: &CertifiedAsset,
    ) -> Option<(
        HttpResponse<'static>,
        ic_certification::HashTree,
        Vec<String>,
    )> {
        // 1. Select encoding based on Accept-Encoding header.
        let encoding = self.select_encoding(request, asset);
        let content = asset.encodings.get(&encoding)?;

        // 2. Build response headers.
        let mut headers = vec![
            ("content-type".to_string(), asset.content_type.clone()),
            (
                CERTIFICATE_EXPRESSION_HEADER_NAME.to_string(),
                asset.cel_expression.clone(),
            ),
        ];
        for (name, value) in &asset.headers {
            headers.push((name.clone(), value.clone()));
        }
        if encoding != AssetEncoding::Identity {
            headers.push((
                "content-encoding".to_string(),
                encoding.as_str().to_string(),
            ));
        }

        let response = HttpResponse::builder()
            .with_status_code(asset.status_code)
            .with_headers(headers)
            .with_body(Cow::<[u8]>::Owned(content.clone()))
            .build();

        // 3. Generate witness and expression path from the certification tree.
        // All modes (including Skip) need a valid witness so the boundary
        // node can verify the certification proof. Skip mode's proof tells
        // the boundary node that the canister intentionally chose not to
        // certify this path.
        let tree = self.tree.borrow();
        let witness = tree.witness(&asset.tree_entry, request_path).ok()?;
        let expr_path = asset.tree_entry.path.to_expr_path();
        Some((response, witness, expr_path))
    }

    fn select_encoding(&self, request: &HttpRequest, asset: &CertifiedAsset) -> AssetEncoding {
        let accept_encoding = request
            .headers()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-encoding"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");

        // Prefer Brotli, then Gzip, then Identity.
        if accept_encoding.contains("br") && asset.encodings.contains_key(&AssetEncoding::Brotli) {
            return AssetEncoding::Brotli;
        }
        if accept_encoding.contains("gzip") && asset.encodings.contains_key(&AssetEncoding::Gzip) {
            return AssetEncoding::Gzip;
        }
        AssetEncoding::Identity
    }

    /// Delete an asset by path (canonical or alias).
    pub fn delete_asset(&mut self, path: &str) {
        let canonical = self
            .aliases
            .remove(path)
            .unwrap_or_else(|| path.to_string());

        if let Some(asset) = self.assets.remove(&canonical) {
            self.tree.borrow_mut().delete(&asset.tree_entry);
            self.fallbacks.retain(|(_, v)| v != &canonical);
            self.aliases.retain(|_, v| v != &canonical);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree() -> Rc<RefCell<HttpCertificationTree>> {
        Rc::new(RefCell::new(HttpCertificationTree::default()))
    }

    fn make_router() -> AssetRouter {
        AssetRouter::with_tree(make_tree())
    }

    fn default_config() -> AssetCertificationConfig {
        AssetCertificationConfig::default()
    }

    fn skip_config() -> AssetCertificationConfig {
        AssetCertificationConfig {
            mode: CertificationMode::skip(),
            ..Default::default()
        }
    }

    fn full_config() -> AssetCertificationConfig {
        AssetCertificationConfig {
            mode: CertificationMode::authenticated(),
            ..Default::default()
        }
    }

    fn make_get_request(url: &str) -> HttpRequest<'_> {
        HttpRequest::get(url.to_string()).build()
    }

    fn make_get_request_with_encoding(url: &str, accept_encoding: &str) -> HttpRequest<'static> {
        HttpRequest::get(url.to_string())
            .with_headers(vec![(
                "accept-encoding".to_string(),
                accept_encoding.to_string(),
            )])
            .build()
    }

    fn make_response(body: &[u8]) -> HttpResponse<'static> {
        HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![("content-type".to_string(), "text/html".to_string())])
            .with_body(Cow::<[u8]>::Owned(body.to_vec()))
            .build()
    }

    // ==================================================================
    // 7.2.11 — certify_asset and certify_dynamic_asset tests
    // ==================================================================

    #[test]
    fn certify_asset_response_only_succeeds() {
        let mut router = make_router();
        let result =
            router.certify_asset("/index.html", b"<h1>Hello</h1>".to_vec(), default_config());
        assert!(result.is_ok());
        assert!(router.contains_asset("/index.html"));
        let asset = router.get_asset("/index.html").unwrap();
        assert_eq!(asset.content, b"<h1>Hello</h1>");
        assert_eq!(asset.content_type, "text/html");
        assert!(matches!(
            asset.certification_mode,
            CertificationMode::ResponseOnly(_)
        ));
    }

    #[test]
    fn certify_asset_skip_succeeds() {
        let mut router = make_router();
        let result = router.certify_asset("/health", b"ok".to_vec(), skip_config());
        assert!(result.is_ok());
        assert!(router.contains_asset("/health"));
        let asset = router.get_asset("/health").unwrap();
        assert!(matches!(asset.certification_mode, CertificationMode::Skip));
    }

    #[test]
    fn certify_asset_full_returns_error() {
        let mut router = make_router();
        let result = router.certify_asset("/api/data", b"{}".to_vec(), full_config());
        assert!(result.is_err());
        match result.unwrap_err() {
            AssetRouterError::FullModeRequiresRequest => {}
            other => panic!("expected FullModeRequiresRequest, got {:?}", other),
        }
    }

    #[test]
    fn certify_dynamic_asset_full_succeeds() {
        let mut router = make_router();
        let request = HttpRequest::get("/api/data".to_string())
            .with_headers(vec![(
                "authorization".to_string(),
                "Bearer token123".to_string(),
            )])
            .build();
        let response = make_response(b"{\"data\": 42}");
        let config = AssetCertificationConfig {
            mode: CertificationMode::authenticated(),
            content_type: Some("application/json".to_string()),
            ..Default::default()
        };

        let result = router.certify_dynamic_asset("/api/data", &request, &response, config);
        assert!(result.is_ok());
        assert!(router.contains_asset("/api/data"));
        let asset = router.get_asset("/api/data").unwrap();
        assert!(matches!(
            asset.certification_mode,
            CertificationMode::Full(_)
        ));
        assert_eq!(asset.content, b"{\"data\": 42}");
    }

    #[test]
    fn certify_dynamic_asset_response_only_succeeds() {
        let mut router = make_router();
        let request = make_get_request("/page");
        let response = make_response(b"page content");
        let result = router.certify_dynamic_asset("/page", &request, &response, default_config());
        assert!(
            result.is_ok(),
            "certify_dynamic_asset failed: {:?}",
            result.err()
        );
        assert!(router.contains_asset("/page"));
    }

    #[test]
    fn certify_dynamic_asset_skip_succeeds() {
        let mut router = make_router();
        let request = make_get_request("/health");
        let response = make_response(b"ok");
        let result = router.certify_dynamic_asset("/health", &request, &response, skip_config());
        assert!(result.is_ok());
        assert!(router.contains_asset("/health"));
    }

    #[test]
    fn certify_asset_auto_detects_content_type() {
        let mut router = make_router();
        router
            .certify_asset("/style.css", b"body {}".to_vec(), default_config())
            .unwrap();
        let asset = router.get_asset("/style.css").unwrap();
        assert_eq!(asset.content_type, "text/css");
    }

    #[test]
    fn certify_asset_uses_explicit_content_type() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            content_type: Some("application/wasm".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/module.bin", b"\0asm".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/module.bin").unwrap();
        assert_eq!(asset.content_type, "application/wasm");
    }

    #[test]
    fn certify_asset_registers_aliases() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            aliases: vec!["/".to_string(), "/home".to_string()],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"<h1>Home</h1>".to_vec(), config)
            .unwrap();
        assert!(router.contains_asset("/index.html"));
        assert!(router.contains_asset("/"));
        assert!(router.contains_asset("/home"));

        // All aliases resolve to the same asset.
        let a1 = router.get_asset("/index.html").unwrap() as *const _;
        let a2 = router.get_asset("/").unwrap() as *const _;
        let a3 = router.get_asset("/home").unwrap() as *const _;
        assert_eq!(a1, a2);
        assert_eq!(a1, a3);
    }

    #[test]
    fn certify_asset_registers_fallback() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            fallback_for: Some("/".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"<h1>SPA</h1>".to_vec(), config)
            .unwrap();
        assert_eq!(router.fallbacks.len(), 1);
        assert_eq!(
            router.fallbacks[0],
            ("/".to_string(), "/index.html".to_string())
        );
    }

    #[test]
    fn certify_asset_stores_encodings() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            encodings: vec![
                (AssetEncoding::Gzip, b"gzip-content".to_vec()),
                (AssetEncoding::Brotli, b"brotli-content".to_vec()),
            ],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"<h1>Hello</h1>".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/index.html").unwrap();
        assert_eq!(asset.encodings.len(), 3); // Identity + Gzip + Brotli
        assert_eq!(
            asset.encodings.get(&AssetEncoding::Identity).unwrap(),
            b"<h1>Hello</h1>"
        );
        assert_eq!(
            asset.encodings.get(&AssetEncoding::Gzip).unwrap(),
            b"gzip-content"
        );
        assert_eq!(
            asset.encodings.get(&AssetEncoding::Brotli).unwrap(),
            b"brotli-content"
        );
    }

    #[test]
    fn certify_asset_stores_certified_at_and_ttl() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            certified_at: 1_000_000,
            ttl: Some(Duration::from_secs(3600)),
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/page").unwrap();
        assert_eq!(asset.certified_at, 1_000_000);
        assert_eq!(asset.ttl, Some(Duration::from_secs(3600)));
    }

    // ==================================================================
    // 7.2.12 — serve_asset tests
    // ==================================================================

    #[test]
    fn serve_asset_exact_match() {
        let mut router = make_router();
        router
            .certify_asset("/index.html", b"<h1>Hello</h1>".to_vec(), default_config())
            .unwrap();

        let request = make_get_request("/index.html");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _witness, expr_path) = result.unwrap();
        assert_eq!(response.status_code(), StatusCode::OK);
        assert_eq!(response.body(), b"<h1>Hello</h1>");
        // expr_path should be non-empty for ResponseOnly mode.
        assert!(!expr_path.is_empty());
    }

    #[test]
    fn serve_asset_alias_resolves() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            aliases: vec!["/".to_string()],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"<h1>Home</h1>".to_vec(), config)
            .unwrap();

        let request = make_get_request("/");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"<h1>Home</h1>");
    }

    #[test]
    fn serve_asset_fallback_match() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            fallback_for: Some("/".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"<h1>SPA</h1>".to_vec(), config)
            .unwrap();

        // Request for a path that doesn't exist should fall back.
        let request = make_get_request("/about");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"<h1>SPA</h1>");
    }

    #[test]
    fn serve_asset_longest_prefix_fallback_wins() {
        let mut router = make_router();

        // Register a general fallback for "/"
        let config1 = AssetCertificationConfig {
            fallback_for: Some("/".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/404.html", b"Not Found".to_vec(), config1)
            .unwrap();

        // Register a more specific fallback for "/app"
        let config2 = AssetCertificationConfig {
            fallback_for: Some("/app".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/app/index.html", b"App SPA".to_vec(), config2)
            .unwrap();

        // Request under /app should match the /app fallback.
        let request = make_get_request("/app/dashboard");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"App SPA");

        // Request outside /app should match the / fallback.
        let request = make_get_request("/about");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"Not Found");
    }

    #[test]
    fn serve_asset_no_match_returns_none() {
        let mut router = make_router();
        router
            .certify_asset("/index.html", b"content".to_vec(), default_config())
            .unwrap();

        let request = make_get_request("/missing");
        let result = router.serve_asset(&request);
        assert!(result.is_none());
    }

    #[test]
    fn serve_asset_skip_has_valid_witness_and_expr_path() {
        let mut router = make_router();
        router
            .certify_asset("/health", b"ok".to_vec(), skip_config())
            .unwrap();

        let request = make_get_request("/health");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _witness, expr_path) = result.unwrap();
        assert_eq!(response.body(), b"ok");
        // Skip mode still has a valid witness and expr_path so the
        // boundary node can verify the skip proof.
        assert!(!expr_path.is_empty());
    }

    #[test]
    fn serve_asset_encoding_negotiation_brotli_preferred() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            encodings: vec![
                (AssetEncoding::Gzip, b"gzip-content".to_vec()),
                (AssetEncoding::Brotli, b"br-content".to_vec()),
            ],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"raw".to_vec(), config)
            .unwrap();

        // Request with both br and gzip -> should get brotli.
        let request = make_get_request_with_encoding("/index.html", "gzip, br");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"br-content");
        // Should have content-encoding: br header.
        let ce = response
            .headers()
            .iter()
            .find(|(k, _)| k == "content-encoding")
            .map(|(_, v)| v.as_str());
        assert_eq!(ce, Some("br"));
    }

    #[test]
    fn serve_asset_encoding_negotiation_gzip_fallback() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            encodings: vec![(AssetEncoding::Gzip, b"gzip-content".to_vec())],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"raw".to_vec(), config)
            .unwrap();

        // Request with only gzip -> should get gzip.
        let request = make_get_request_with_encoding("/index.html", "gzip");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"gzip-content");
    }

    #[test]
    fn serve_asset_encoding_negotiation_identity_fallback() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            encodings: vec![(AssetEncoding::Brotli, b"br-content".to_vec())],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"raw".to_vec(), config)
            .unwrap();

        // Request without accept-encoding -> should get identity.
        let request = make_get_request("/index.html");
        let result = router.serve_asset(&request);
        assert!(result.is_some());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.body(), b"raw");
        // Should NOT have content-encoding header.
        let ce = response
            .headers()
            .iter()
            .find(|(k, _)| k == "content-encoding");
        assert!(ce.is_none());
    }

    #[test]
    fn serve_asset_includes_cel_expression_header() {
        let mut router = make_router();
        router
            .certify_asset("/index.html", b"content".to_vec(), default_config())
            .unwrap();

        let request = make_get_request("/index.html");
        let (response, _, _) = router.serve_asset(&request).unwrap();
        let cel_header = response
            .headers()
            .iter()
            .find(|(k, _)| k == CERTIFICATE_EXPRESSION_HEADER_NAME)
            .map(|(_, v)| v.clone());
        assert!(cel_header.is_some());
        assert!(!cel_header.unwrap().is_empty());
    }

    // ==================================================================
    // 7.2.13 — delete_asset, root_hash, re-certification, mode switching
    // ==================================================================

    #[test]
    fn delete_asset_removes_asset_and_tree_entry() {
        let mut router = make_router();
        router
            .certify_asset("/page", b"content".to_vec(), default_config())
            .unwrap();
        assert!(router.contains_asset("/page"));

        router.delete_asset("/page");
        assert!(!router.contains_asset("/page"));
        assert!(router.get_asset("/page").is_none());
    }

    #[test]
    fn delete_asset_removes_aliases() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            aliases: vec!["/home".to_string()],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"content".to_vec(), config)
            .unwrap();
        assert!(router.contains_asset("/home"));

        router.delete_asset("/index.html");
        assert!(!router.contains_asset("/index.html"));
        assert!(!router.contains_asset("/home"));
    }

    #[test]
    fn delete_asset_removes_fallback() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            fallback_for: Some("/".to_string()),
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"content".to_vec(), config)
            .unwrap();
        assert_eq!(router.fallbacks.len(), 1);

        router.delete_asset("/index.html");
        assert!(router.fallbacks.is_empty());
    }

    #[test]
    fn delete_asset_via_alias_resolves_and_removes_canonical() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            aliases: vec!["/home".to_string()],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"content".to_vec(), config)
            .unwrap();

        // Delete via alias.
        router.delete_asset("/home");
        assert!(!router.contains_asset("/index.html"));
        assert!(!router.contains_asset("/home"));
    }

    #[test]
    fn delete_asset_nonexistent_is_noop() {
        let mut router = make_router();
        // Should not panic.
        router.delete_asset("/nonexistent");
        assert!(!router.contains_asset("/nonexistent"));
    }

    #[test]
    fn get_asset_via_canonical_and_alias() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            aliases: vec!["/home".to_string()],
            ..Default::default()
        };
        router
            .certify_asset("/index.html", b"content".to_vec(), config)
            .unwrap();

        assert!(router.get_asset("/index.html").is_some());
        assert!(router.get_asset("/home").is_some());
    }

    #[test]
    fn get_asset_nonexistent_returns_none() {
        let router = make_router();
        assert!(router.get_asset("/nonexistent").is_none());
    }

    #[test]
    fn root_hash_changes_after_certify() {
        let tree = make_tree();
        let mut router = AssetRouter::with_tree(tree);
        let hash_before = router.root_hash();

        router
            .certify_asset("/page", b"content".to_vec(), default_config())
            .unwrap();
        let hash_after = router.root_hash();

        assert_ne!(hash_before, hash_after);
    }

    #[test]
    fn root_hash_changes_after_delete() {
        let tree = make_tree();
        let mut router = AssetRouter::with_tree(tree);

        router
            .certify_asset("/page", b"content".to_vec(), default_config())
            .unwrap();
        let hash_with_asset = router.root_hash();

        router.delete_asset("/page");
        let hash_after_delete = router.root_hash();

        assert_ne!(hash_with_asset, hash_after_delete);
    }

    #[test]
    fn recertification_replaces_old_hash() {
        let tree = make_tree();
        let mut router = AssetRouter::with_tree(tree);

        router
            .certify_asset("/page", b"v1".to_vec(), default_config())
            .unwrap();
        let hash_v1 = router.root_hash();

        // Delete and re-certify with different content.
        router.delete_asset("/page");
        router
            .certify_asset("/page", b"v2".to_vec(), default_config())
            .unwrap();
        let hash_v2 = router.root_hash();

        assert_ne!(hash_v1, hash_v2);
    }

    #[test]
    fn mode_switching_certify_delete_recertify() {
        let tree = make_tree();
        let mut router = AssetRouter::with_tree(tree);

        // Certify with ResponseOnly.
        router
            .certify_asset("/page", b"content".to_vec(), default_config())
            .unwrap();
        assert!(matches!(
            router.get_asset("/page").unwrap().certification_mode,
            CertificationMode::ResponseOnly(_)
        ));

        // Delete and re-certify with Skip.
        router.delete_asset("/page");
        router
            .certify_asset("/page", b"content".to_vec(), skip_config())
            .unwrap();
        assert!(matches!(
            router.get_asset("/page").unwrap().certification_mode,
            CertificationMode::Skip
        ));
    }

    // ==================================================================
    // 7.2.14 — is_dynamic, is_expired tests
    // ==================================================================

    #[test]
    fn is_dynamic_true_when_marked_dynamic() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            ttl: Some(Duration::from_secs(3600)),
            dynamic: true,
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        assert!(router.get_asset("/page").unwrap().is_dynamic());
    }

    #[test]
    fn is_dynamic_false_when_not_marked_dynamic() {
        let mut router = make_router();
        router
            .certify_asset("/page", b"content".to_vec(), default_config())
            .unwrap();
        assert!(!router.get_asset("/page").unwrap().is_dynamic());
    }

    #[test]
    fn is_dynamic_true_without_ttl() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            ttl: None,
            dynamic: true,
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        // Dynamic flag is independent of TTL.
        assert!(router.get_asset("/page").unwrap().is_dynamic());
    }

    #[test]
    fn is_expired_respects_certified_at_and_ttl() {
        let mut router = make_router();
        let one_hour_ns: u64 = 3_600_000_000_000;
        let config = AssetCertificationConfig {
            certified_at: 1_000_000,
            ttl: Some(Duration::from_secs(3600)),
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/page").unwrap();

        // Before expiry.
        assert!(!asset.is_expired(1_000_000 + one_hour_ns - 1));
        // At expiry boundary.
        assert!(asset.is_expired(1_000_000 + one_hour_ns));
        // After expiry.
        assert!(asset.is_expired(1_000_000 + one_hour_ns + 1));
    }

    #[test]
    fn is_expired_static_assets_never_expire() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            certified_at: 1_000_000,
            ttl: None,
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/page").unwrap();

        assert!(!asset.is_expired(u64::MAX));
        assert!(!asset.is_expired(0));
    }

    // ==================================================================
    // Additional type / helper tests
    // ==================================================================

    #[test]
    fn asset_encoding_as_str() {
        assert_eq!(AssetEncoding::Identity.as_str(), "identity");
        assert_eq!(AssetEncoding::Gzip.as_str(), "gzip");
        assert_eq!(AssetEncoding::Brotli.as_str(), "br");
    }

    #[test]
    fn asset_certification_config_default() {
        let config = AssetCertificationConfig::default();
        assert!(matches!(config.mode, CertificationMode::ResponseOnly(_)));
        assert!(config.content_type.is_none());
        assert!(config.headers.is_empty());
        assert!(config.encodings.is_empty());
        assert!(config.fallback_for.is_none());
        assert!(config.aliases.is_empty());
        assert_eq!(config.certified_at, 0);
        assert!(config.ttl.is_none());
    }

    #[test]
    fn asset_router_error_display() {
        let e = AssetRouterError::CertificationFailed("bad".into());
        assert!(e.to_string().contains("bad"));

        let e = AssetRouterError::FullModeRequiresRequest;
        assert!(e.to_string().contains("certify_dynamic_asset"));

        let e = AssetRouterError::AssetNotFound("/missing".into());
        assert!(e.to_string().contains("/missing"));
    }

    #[test]
    fn new_router_is_empty() {
        let router = make_router();
        assert!(!router.contains_asset("/anything"));
        assert!(router.get_asset("/anything").is_none());
    }

    #[test]
    fn get_asset_mut_works() {
        let mut router = make_router();
        router
            .certify_asset("/page", b"original".to_vec(), default_config())
            .unwrap();

        let asset = router.get_asset_mut("/page").unwrap();
        asset.certified_at = 999;
        assert_eq!(router.get_asset("/page").unwrap().certified_at, 999);
    }

    #[test]
    fn build_cel_expression_string_skip() {
        let mode = CertificationMode::skip();
        let cel = build_cel_expression_string(&mode);
        assert!(!cel.is_empty());
    }

    #[test]
    fn build_cel_expression_string_response_only() {
        let mode = CertificationMode::response_only();
        let cel = build_cel_expression_string(&mode);
        assert!(!cel.is_empty());
        // Should contain response certification markers.
        assert!(cel.contains("certification"));
    }

    #[test]
    fn build_cel_expression_string_full() {
        let mode = CertificationMode::authenticated();
        let cel = build_cel_expression_string(&mode);
        assert!(!cel.is_empty());
        assert!(cel.contains("certification"));
    }

    #[test]
    fn certify_asset_with_additional_headers() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            headers: vec![("x-custom".to_string(), "value".to_string())],
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();
        let asset = router.get_asset("/page").unwrap();
        assert_eq!(asset.headers.len(), 1);
        assert_eq!(
            asset.headers[0],
            ("x-custom".to_string(), "value".to_string())
        );
    }

    #[test]
    fn serve_asset_includes_additional_headers() {
        let mut router = make_router();
        let config = AssetCertificationConfig {
            headers: vec![("x-custom".to_string(), "value".to_string())],
            ..Default::default()
        };
        router
            .certify_asset("/page", b"content".to_vec(), config)
            .unwrap();

        let request = make_get_request("/page");
        let (response, _, _) = router.serve_asset(&request).unwrap();
        let custom = response
            .headers()
            .iter()
            .find(|(k, _)| k == "x-custom")
            .map(|(_, v)| v.as_str());
        assert_eq!(custom, Some("value"));
    }

    // ---- 8.6.3: Asset certification edge case tests ----

    /// Re-certifying the same path replaces the old asset entry.
    #[test]
    fn certify_asset_duplicate_path_replaces() {
        let mut router = make_router();

        router
            .certify_asset("/page", b"v1".to_vec(), default_config())
            .unwrap();
        let asset_v1 = router.get_asset("/page").unwrap();
        assert_eq!(asset_v1.content, b"v1");

        // Certify again with different content — should replace, not duplicate.
        router
            .certify_asset("/page", b"v2".to_vec(), default_config())
            .unwrap();
        let asset_v2 = router.get_asset("/page").unwrap();
        assert_eq!(asset_v2.content, b"v2");

        // Serve returns the new content.
        let request = make_get_request("/page");
        let (response, _, _) = router.serve_asset(&request).unwrap();
        assert_eq!(response.body(), b"v2");
    }

    /// Deleting a nonexistent path is a no-op — no panic, no state corruption.
    #[test]
    fn delete_nonexistent_asset_is_noop() {
        let mut router = make_router();
        // Certify one asset to ensure the router is non-empty.
        router
            .certify_asset("/exists", b"data".to_vec(), default_config())
            .unwrap();

        // Deleting a path that was never certified should not panic.
        router.delete_asset("/does-not-exist");

        // The existing asset is unaffected.
        assert!(router.contains_asset("/exists"));
        assert!(!router.contains_asset("/does-not-exist"));
    }
}

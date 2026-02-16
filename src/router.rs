use ic_http_certification::{HttpRequest, HttpResponse, Method};
use std::collections::HashMap;

use crate::middleware::MiddlewareFn;

/// Dynamic route parameters extracted from the URL path.
///
/// Maps parameter names to their captured values. For example, a route
/// registered as `/:postId/edit` matched against `/42/edit` produces
/// `{"postId": "42"}`. Wildcard routes store the remaining path under
/// the key `"*"`.
pub type RouteParams = HashMap<String, String>;

/// A synchronous route handler function.
///
/// Receives the full [`HttpRequest`] and the extracted [`RouteParams`],
/// and returns an [`HttpResponse`]. This is the standard handler signature
/// used by the router's middleware chain and certification pipeline.
pub type HandlerFn = fn(HttpRequest, RouteParams) -> HttpResponse<'static>;

/// Result of matching a path against the route tree (without method dispatch).
///
/// Contains references to the handler maps and the extracted route parameters.
type MatchResult<'a> = (
    &'a HashMap<Method, HandlerFn>,
    &'a HashMap<Method, HandlerResultFn>,
    RouteParams,
);

/// A route handler that returns [`HandlerResult`] instead of a bare response.
///
/// This variant supports conditional regeneration: the handler can return
/// [`HandlerResult::NotModified`] to signal that the existing cached version
/// is still valid, avoiding a full recertification cycle.
///
/// A standard [`HandlerFn`] must also be registered at the same path/method
/// as a fallback for the query path and middleware chain. The
/// `HandlerResultFn` is only called during `http_request_update`.
///
/// # Note on signature
///
/// This type intentionally uses the internal `(HttpRequest, RouteParams)`
/// signature rather than the public `RouteContext`-based signature used by
/// generated route handlers. The build script currently does not generate
/// wrappers for `insert_result` calls — result handlers are registered
/// manually via [`RouteNode::insert_result`].
///
/// If `insert_result` is ever wired into `__route_tree.rs` code generation,
/// the same wrapper pattern used for `HandlerFn` (bridging `RouteParams` to
/// `RouteContext<Params, SearchParams>`) should be applied here as well.
pub type HandlerResultFn = fn(HttpRequest, RouteParams) -> HandlerResult;

/// Result type for route handlers that supports conditional regeneration.
///
/// Handlers can return `Response(...)` with a new response to certify, or
/// `NotModified` to indicate that the existing cached version is still valid.
/// When `NotModified` is returned, the library skips recertification entirely
/// and resets the TTL timer if TTL-based caching is active.
///
/// # Examples
///
/// ```rust,ignore
/// use ic_asset_router::HandlerResult;
/// use ic_http_certification::{HttpRequest, HttpResponse};
/// use ic_asset_router::router::RouteParams;
///
/// fn my_result_handler(req: HttpRequest, params: RouteParams) -> HandlerResult {
///     if content_unchanged() {
///         HandlerResult::NotModified
///     } else {
///         HandlerResult::Response(build_new_response())
///     }
/// }
/// ```
pub enum HandlerResult {
    /// New content — certify and cache this response.
    Response(HttpResponse<'static>),

    /// Content hasn't changed — keep the existing certified version
    /// and reset the TTL timer (if TTL-based caching is enabled).
    NotModified,
}

impl From<HttpResponse<'static>> for HandlerResult {
    fn from(resp: HttpResponse<'static>) -> Self {
        HandlerResult::Response(resp)
    }
}

/// The type of a node in the route trie.
///
/// Each segment of a route path corresponds to a [`RouteNode`] with one of
/// these types. During path resolution the trie tries `Static` first, then
/// `Param`, then `Wildcard` — giving static segments the highest priority.
#[derive(Debug, PartialEq, Eq)]
pub enum NodeType {
    /// A literal path segment (e.g. `"users"` in `/users`).
    Static(String),
    /// A dynamic parameter segment (e.g. `:id` in `/users/:id`).
    /// The contained string is the parameter name without the leading colon.
    Param(String),
    /// A catch-all wildcard (`*`). Matches one or more remaining segments
    /// and stores the captured tail in [`RouteParams`] under the key `"*"`.
    Wildcard,
}

/// Result of resolving a path and HTTP method against the route tree.
///
/// Returned by [`RouteNode::resolve`]. Callers should match on the three
/// variants to determine how to handle the request.
pub enum RouteResult {
    /// A handler was found for the given path and method.
    /// The optional `HandlerResultFn` is present when the route supports
    /// conditional regeneration via `HandlerResult::NotModified`.
    Found(HandlerFn, RouteParams, Option<HandlerResultFn>),
    /// The path exists but the requested method is not registered.
    /// Contains the list of methods that *are* registered for this path.
    MethodNotAllowed(Vec<Method>),
    /// No route matches the given path.
    NotFound,
}

/// A node in the radix-trie-style route tree.
///
/// The root node is always `NodeType::Static("")`. Routes are inserted by
/// splitting the path into segments and descending through (or creating)
/// child nodes. Resolution follows the same path, preferring static matches
/// over parameter matches over wildcard matches.
///
/// Middleware, the custom not-found handler, and `HandlerResultFn` overrides
/// are stored on the root node and consulted at dispatch time.
pub struct RouteNode {
    /// The type of this node (static segment, parameter, or wildcard).
    pub node_type: NodeType,
    /// Child nodes forming the rest of the trie.
    pub children: Vec<RouteNode>,
    /// Method → handler map for this node. A handler is present only for
    /// methods that have been explicitly registered via [`insert`](Self::insert).
    pub handlers: HashMap<Method, HandlerFn>,
    /// Optional `HandlerResultFn` overrides for routes that support conditional
    /// regeneration. When present for a given method, `http_request_update` calls
    /// this handler first to check for `NotModified` before falling back to the
    /// standard `HandlerFn` + middleware pipeline.
    result_handlers: HashMap<Method, HandlerResultFn>,
    /// Middleware registry stored at the root node.
    /// Each entry is a `(prefix, middleware_fn)` pair, sorted by prefix segment
    /// count (shortest/outermost first). Only the root node's list is used at
    /// dispatch time; child nodes ignore this field.
    middlewares: Vec<(String, MiddlewareFn)>,
    /// Optional custom not-found handler. When set, this handler is called
    /// instead of the default 404 response when no route matches the request
    /// path. Only the root node's value is used at dispatch time.
    not_found_handler: Option<HandlerFn>,
}

impl RouteNode {
    /// Create a new route node with the given type and no children or handlers.
    pub fn new(node_type: NodeType) -> Self {
        Self {
            node_type,
            children: Vec::new(),
            handlers: HashMap::new(),
            result_handlers: HashMap::new(),
            middlewares: Vec::new(),
            not_found_handler: None,
        }
    }

    /// Register a middleware at the given prefix.
    ///
    /// One middleware per prefix — calling this again with the same prefix
    /// replaces the previous middleware. The list is kept sorted by prefix
    /// segment count (shortest/outermost first) so that the middleware chain
    /// executes in root → outer → inner order.
    ///
    /// Use `"/"` for root-level middleware that runs on every request.
    pub fn set_middleware(&mut self, prefix: &str, mw: MiddlewareFn) {
        let normalized = normalize_prefix(prefix);
        if let Some(entry) = self.middlewares.iter_mut().find(|(p, _)| *p == normalized) {
            entry.1 = mw;
        } else {
            self.middlewares.push((normalized, mw));
        }
        // Sort by segment count (shortest first) for correct outer → inner ordering.
        self.middlewares.sort_by_key(|(p, _)| segment_count(p));
    }

    /// Register a custom not-found handler.
    ///
    /// When no route matches a request path, this handler is called instead of
    /// returning the default plain-text 404 response. The handler receives the
    /// full `HttpRequest` and empty `RouteParams`.
    ///
    /// Only one not-found handler can be registered; calling this again replaces
    /// the previous handler.
    pub fn set_not_found(&mut self, handler: HandlerFn) {
        self.not_found_handler = Some(handler);
    }

    /// Returns the custom not-found handler, if one has been registered.
    pub fn not_found_handler(&self) -> Option<HandlerFn> {
        self.not_found_handler
    }

    /// Register a handler for the given path and HTTP method.
    ///
    /// Path segments starting with `:` are treated as dynamic parameters;
    /// a lone `*` segment is a catch-all wildcard. If a handler already
    /// exists for the same path and method it is silently replaced.
    pub fn insert(&mut self, path: &str, method: Method, handler: HandlerFn) {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._insert(&segments, method, handler);
    }

    /// Register a `HandlerResultFn` for the given path and method.
    ///
    /// A `HandlerResultFn` returns `HandlerResult` instead of `HttpResponse`,
    /// enabling conditional regeneration via `HandlerResult::NotModified`.
    ///
    /// A standard `HandlerFn` must also be registered at the same path/method
    /// (via [`insert`](Self::insert)) — it serves as the fallback for the query
    /// path and middleware chain. The `HandlerResultFn` is only checked in
    /// `http_request_update`.
    pub fn insert_result(&mut self, path: &str, method: Method, handler: HandlerResultFn) {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._insert_result(&segments, method, handler);
    }

    fn _insert(&mut self, segments: &[&str], method: Method, handler: HandlerFn) {
        if segments.is_empty() {
            self.handlers.insert(method, handler);
            return;
        }

        let node_type = match segments[0] {
            "*" => NodeType::Wildcard,
            s if s.starts_with(':') => NodeType::Param(s[1..].to_string()),
            s => NodeType::Static(s.to_string()),
        };

        let child = self.children.iter_mut().find(|c| c.node_type == node_type);

        match child {
            Some(c) => c._insert(&segments[1..], method, handler),
            None => {
                let mut new_node = RouteNode::new(node_type);
                new_node._insert(&segments[1..], method, handler);
                self.children.push(new_node);
            }
        }
    }

    fn _insert_result(&mut self, segments: &[&str], method: Method, handler: HandlerResultFn) {
        if segments.is_empty() {
            self.result_handlers.insert(method, handler);
            return;
        }

        let node_type = match segments[0] {
            "*" => NodeType::Wildcard,
            s if s.starts_with(':') => NodeType::Param(s[1..].to_string()),
            s => NodeType::Static(s.to_string()),
        };

        let child = self.children.iter_mut().find(|c| c.node_type == node_type);

        match child {
            Some(c) => c._insert_result(&segments[1..], method, handler),
            None => {
                let mut new_node = RouteNode::new(node_type);
                new_node._insert_result(&segments[1..], method, handler);
                self.children.push(new_node);
            }
        }
    }

    /// Execute the middleware chain for a resolved route.
    ///
    /// Collects all middleware whose prefix matches `path` (sorted outermost
    /// first), wraps `handler` as the innermost `next`, and executes the chain.
    /// Each middleware's `next` calls the next middleware inward, with the
    /// handler at the center.
    pub fn execute_with_middleware(
        &self,
        path: &str,
        handler: HandlerFn,
        req: HttpRequest,
        params: RouteParams,
    ) -> HttpResponse<'static> {
        let matching: Vec<MiddlewareFn> = self
            .middlewares
            .iter()
            .filter(|(prefix, _)| path_matches_prefix(path, prefix))
            .map(|(_, mw)| *mw)
            .collect();

        if matching.is_empty() {
            return handler(req, params);
        }

        // Build the chain from innermost to outermost.
        // Start with the handler as the innermost function.
        // Then wrap each middleware around it, from the last (innermost) to the
        // first (outermost).
        build_chain(&matching, handler, req, &params)
    }

    /// Execute the middleware chain for a not-found request.
    ///
    /// This is used when a custom not-found handler is registered: the
    /// middleware chain still runs (root/global middleware should execute
    /// before the 404 handler), with the not-found handler at the center
    /// instead of a route handler.
    pub fn execute_not_found_with_middleware(
        &self,
        path: &str,
        req: HttpRequest,
    ) -> Option<HttpResponse<'static>> {
        let handler = self.not_found_handler?;
        let params = RouteParams::new();
        Some(self.execute_with_middleware(path, handler, req, params))
    }

    /// Resolve a path and method to a `RouteResult`.
    ///
    /// 1. Finds the trie node matching `path`.
    /// 2. If found, looks up `method` in the node's `handlers` map.
    /// 3. Returns `Found` / `MethodNotAllowed` / `NotFound` accordingly.
    pub fn resolve(&self, path: &str, method: &Method) -> RouteResult {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        match self._match(&segments) {
            Some((handlers, result_handlers, params)) => {
                if let Some(&handler) = handlers.get(method) {
                    let result_handler = result_handlers.get(method).copied();
                    RouteResult::Found(handler, params, result_handler)
                } else {
                    let allowed: Vec<Method> = handlers.keys().cloned().collect();
                    RouteResult::MethodNotAllowed(allowed)
                }
            }
            None => RouteResult::NotFound,
        }
    }

    /// Match a path and return the handlers map and params for the matched node.
    ///
    /// This performs path-only matching without method dispatch.
    /// For method-aware routing, use [`resolve()`](Self::resolve) instead.
    pub fn match_path(&self, path: &str) -> Option<MatchResult<'_>> {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._match(&segments)
    }

    fn _match(&self, segments: &[&str]) -> Option<MatchResult<'_>> {
        if segments.is_empty() {
            if !self.handlers.is_empty() {
                return Some((&self.handlers, &self.result_handlers, HashMap::new()));
            }
            // No handlers on this node — check for a wildcard child (empty wildcard match)
            for child in &self.children {
                if let NodeType::Wildcard = child.node_type {
                    if !child.handlers.is_empty() {
                        let mut params = HashMap::new();
                        params.insert("*".to_string(), String::new());
                        return Some((&child.handlers, &child.result_handlers, params));
                    }
                }
            }
            return None;
        }

        let head = segments[0];
        let tail = &segments[1..];

        debug_log!("head: {:?}", head);

        // Static match
        for child in &self.children {
            if let NodeType::Static(ref s) = child.node_type {
                if s == head {
                    if let Some((h, rh, p)) = child._match(tail) {
                        debug_log!("Static match: {:?}", segments);
                        return Some((h, rh, p));
                    }
                }
            }
        }

        // Param match
        for child in &self.children {
            if let NodeType::Param(ref name) = child.node_type {
                if let Some((h, rh, mut p)) = child._match(tail) {
                    p.insert(name.clone(), head.to_string());
                    debug_log!("Param match: {:?}", segments);
                    return Some((h, rh, p));
                }
            }
        }

        // Wildcard match
        for child in &self.children {
            if let NodeType::Wildcard = child.node_type {
                if !segments.is_empty() && !child.handlers.is_empty() {
                    debug_log!("Wildcard match: {:?}", segments);
                    let remaining = segments.join("/");
                    let mut params = HashMap::new();
                    params.insert("*".to_string(), remaining);
                    return Some((&child.handlers, &child.result_handlers, params));
                }
            }
        }

        None
    }
}

/// Check whether a request path matches a middleware prefix.
///
/// `"/"` matches all paths. Otherwise, the path must start with the prefix
/// followed by either end-of-string or a `"/"` separator.
fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    if prefix == "/" {
        return true;
    }
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Build and execute a nested middleware chain.
///
/// `middlewares` is sorted outermost-first. The handler is the innermost
/// function. We recurse: middleware[0] wraps a `next` that calls
/// `build_chain(middlewares[1..], handler, ...)`.
fn build_chain(
    middlewares: &[MiddlewareFn],
    handler: HandlerFn,
    req: HttpRequest,
    params: &RouteParams,
) -> HttpResponse<'static> {
    match middlewares.split_first() {
        None => handler(req, params.clone()),
        Some((&mw, rest)) => {
            let next =
                |inner_req: HttpRequest, inner_params: &RouteParams| -> HttpResponse<'static> {
                    build_chain(rest, handler, inner_req, inner_params)
                };
            mw(req, params, &next)
        }
    }
}

/// Normalize a middleware prefix to a canonical form: `"/"` for root, otherwise
/// `"/segment1/segment2"` with no trailing slash.
fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed}")
    }
}

/// Count the number of non-empty path segments in a normalized prefix.
/// `"/"` has 0 segments; `"/api"` has 1; `"/api/v2"` has 2.
fn segment_count(prefix: &str) -> usize {
    prefix.split('/').filter(|s| !s.is_empty()).count()
}

// Test coverage audit (Session 7, Spec 5.5):
//
// Covered:
//   - Basic path matching: root, static, dynamic, wildcard, nested params, mixed params+wildcard
//   - Trailing slash normalization, double slash normalization
//   - Method dispatch: GET/POST differentiation, MethodNotAllowed with allowed list, all 7 methods
//   - Middleware: root scope, scoped prefix, chain order (root→outer→inner), short-circuit,
//     response modification, replacement on same prefix, query+update paths
//   - Custom 404: custom response, default fallback, request pass-through, JSON content-type,
//     middleware runs before 404
//   - From<HttpResponse> for HandlerResult conversion
//
// Gaps filled in this session:
//   - Empty segments in paths
//   - URL-encoded characters in path segments
//   - Very long paths (100 segments)
//   - Routes with many (4+) parameters
//   - Middleware modifying request before handler (header injection)
//   - Multiple middleware in hierarchy applied to not-found handler
#[cfg(test)]
mod tests {
    use super::*;
    use ic_http_certification::{Method, StatusCode};
    use std::{borrow::Cow, str};

    fn test_request(path: &str) -> HttpRequest<'_> {
        HttpRequest::builder()
            .with_method(Method::GET)
            .with_url(path)
            .build()
    }

    fn response_with_text(text: &str) -> HttpResponse<'static> {
        HttpResponse::builder()
            .with_body(Cow::Owned(text.as_bytes().to_vec()))
            .with_status_code(StatusCode::OK)
            .build()
    }

    /// Resolve a path as GET and unwrap the Found variant, returning (handler, params).
    fn resolve_get(root: &RouteNode, path: &str) -> (HandlerFn, RouteParams) {
        match root.resolve(path, &Method::GET) {
            RouteResult::Found(h, p, _) => (h, p),
            other => panic!(
                "expected Found for GET {path}, got {}",
                route_result_name(&other)
            ),
        }
    }

    fn route_result_name(r: &RouteResult) -> &'static str {
        match r {
            RouteResult::Found(_, _, _) => "Found",
            RouteResult::MethodNotAllowed(_) => "MethodNotAllowed",
            RouteResult::NotFound => "NotFound",
        }
    }

    fn matched_root(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("root")
    }

    fn matched_404(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("404")
    }

    fn matched_index2(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("index2")
    }

    fn matched_about(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("about")
    }

    fn matched_deep(_: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
        response_with_text(&format!("deep: {params:?}"))
    }

    fn matched_folder(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("folder")
    }

    fn setup_router() -> RouteNode {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/", Method::GET, matched_root);
        root.insert("/*", Method::GET, matched_404);
        root.insert("/index2", Method::GET, matched_index2);
        root.insert("/about", Method::GET, matched_about);
        root.insert("/deep/:pageId", Method::GET, matched_deep);
        root.insert("/deep/:pageId/:subpageId", Method::GET, matched_deep);
        root.insert("/alsodeep/:pageId/edit", Method::GET, matched_deep);
        root.insert("/folder/*", Method::GET, matched_folder);
        root
    }

    fn body_str(resp: HttpResponse<'static>) -> String {
        str::from_utf8(resp.body())
            .unwrap_or("<invalid utf-8>")
            .to_string()
    }

    // ---- Existing path-matching tests (updated for method-aware API) ----

    #[test]
    fn test_root_match() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/");
        assert_eq!(body_str(handler(test_request("/"), params)), "root");
    }

    #[test]
    fn test_404_match() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "/nonexistent");
        assert_eq!(
            body_str(handler(test_request("/nonexistent"), HashMap::new())),
            "404"
        );
    }

    #[test]
    fn test_exact_match() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/index2");
        assert_eq!(body_str(handler(test_request("/index2"), params)), "index2");
    }

    #[test]
    fn test_pathless_layout_route_a() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/about", Method::GET, matched_about);
        let (handler, params) = resolve_get(&root, "/about");
        assert_eq!(body_str(handler(test_request("/about"), params)), "about");
    }

    #[test]
    fn test_dynamic_match() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/deep/page1");
        let body = body_str(handler(test_request("/deep/page1"), params));
        assert!(body.contains("page1"));
    }

    #[test]
    fn test_posts_postid_edit() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/alsodeep/page1/edit");
        let body = body_str(handler(test_request("/alsodeep/page1/edit"), params));
        assert!(body.contains("page1"));
    }

    #[test]
    fn test_nested_dynamic_match() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/deep/page2/subpage1");
        let body = body_str(handler(test_request("/deep/page2/subpage1"), params));
        assert!(body.contains("page2"));
        assert!(body.contains("subpage1"));
    }

    #[test]
    fn test_wildcard_match() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "/folder/anything");
        assert_eq!(
            body_str(handler(test_request("/folder/anything"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_folder_root_wildcard_match() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "/folder/any");
        assert_eq!(
            body_str(handler(test_request("/folder/any"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_deep_wildcard_multi_segments() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "/folder/a/b/c/d");
        assert_eq!(
            body_str(handler(test_request("/folder/a/b/c/d"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_trailing_slash_static_match() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "/index2/");
        assert_eq!(
            body_str(handler(test_request("/index2/"), HashMap::new())),
            "index2"
        );
    }

    #[test]
    fn test_double_slash_matches_normalized() {
        let root = setup_router();
        let (handler, _) = resolve_get(&root, "//index2");
        assert_eq!(
            body_str(handler(test_request("//index2"), HashMap::new())),
            "index2"
        );
    }

    #[test]
    fn test_root_wildcard_captures_full_path() {
        let root = setup_router();
        let (_, params) = resolve_get(&root, "/a/b/c");
        assert_eq!(params.get("*").unwrap(), "a/b/c");
    }

    #[test]
    fn test_folder_wildcard_captures_tail() {
        let root = setup_router();
        let (handler, params) = resolve_get(&root, "/folder/docs/report.pdf");
        assert_eq!(params.get("*").unwrap(), "docs/report.pdf");
        assert_eq!(
            body_str(handler(
                test_request("/folder/docs/report.pdf"),
                params.clone()
            )),
            "folder"
        );
    }

    fn matched_user_files(_: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
        response_with_text(&format!("user_files: {params:?}"))
    }

    #[test]
    fn test_mixed_params_and_wildcard() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/users/:id/files/*", Method::GET, matched_user_files);
        let (_, params) = resolve_get(&root, "/users/42/files/docs/report.pdf");
        assert_eq!(params.get("id").unwrap(), "42");
        assert_eq!(params.get("*").unwrap(), "docs/report.pdf");
    }

    #[test]
    fn test_empty_wildcard_match() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/files/*", Method::GET, matched_folder);
        let (handler, params) = resolve_get(&root, "/files/");
        assert_eq!(params.get("*").unwrap(), "");
        assert_eq!(
            body_str(handler(test_request("/files/"), params.clone())),
            "folder"
        );
    }

    // ---- 2.1 Method dispatch tests ----

    fn matched_post_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("post_handler")
    }

    fn matched_get_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        response_with_text("get_handler")
    }

    /// 2.1.7a: GET /path routes to get handler, POST /path routes to post handler
    #[test]
    fn test_method_dispatch_get_and_post() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/users", Method::GET, matched_get_handler);
        root.insert("/api/users", Method::POST, matched_post_handler);

        // GET resolves to get handler
        match root.resolve("/api/users", &Method::GET) {
            RouteResult::Found(handler, params, _) => {
                assert_eq!(
                    body_str(handler(test_request("/api/users"), params)),
                    "get_handler"
                );
            }
            other => panic!("expected Found, got {}", route_result_name(&other)),
        }

        // POST resolves to post handler
        match root.resolve("/api/users", &Method::POST) {
            RouteResult::Found(handler, params, _) => {
                let req = HttpRequest::builder()
                    .with_method(Method::POST)
                    .with_url("/api/users")
                    .build();
                assert_eq!(body_str(handler(req, params)), "post_handler");
            }
            other => panic!("expected Found, got {}", route_result_name(&other)),
        }
    }

    /// 2.1.7b: PUT /path returns 405 with allowed methods when only GET and POST registered
    #[test]
    fn test_method_not_allowed() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/users", Method::GET, matched_get_handler);
        root.insert("/api/users", Method::POST, matched_post_handler);

        match root.resolve("/api/users", &Method::PUT) {
            RouteResult::MethodNotAllowed(allowed) => {
                let mut names: Vec<&str> = allowed.iter().map(|m| m.as_str()).collect();
                names.sort();
                assert_eq!(names, vec!["GET", "POST"]);
            }
            other => panic!(
                "expected MethodNotAllowed, got {}",
                route_result_name(&other)
            ),
        }
    }

    /// 2.1.7c: Unknown path returns NotFound
    #[test]
    fn test_unknown_path_returns_not_found() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/users", Method::GET, matched_get_handler);

        assert!(matches!(
            root.resolve("/api/nonexistent", &Method::GET),
            RouteResult::NotFound
        ));
    }

    /// 2.1.7d: All 7 HTTP method types can be registered and resolved
    #[test]
    fn test_all_seven_methods() {
        let methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
        ];

        let mut root = RouteNode::new(NodeType::Static("".into()));
        for method in &methods {
            root.insert("/test", method.clone(), matched_get_handler);
        }

        // All 7 methods should resolve to Found
        for method in &methods {
            match root.resolve("/test", method) {
                RouteResult::Found(_, _, _) => {}
                other => panic!(
                    "expected Found for method {}, got {}",
                    method.as_str(),
                    route_result_name(&other)
                ),
            }
        }
    }

    // ---- 2.2 Middleware tests ----

    use std::cell::RefCell;

    thread_local! {
        static LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
    }

    fn clear_log() {
        LOG.with(|l| l.borrow_mut().clear());
    }

    fn get_log() -> Vec<String> {
        LOG.with(|l| l.borrow().clone())
    }

    fn log_entry(msg: &str) {
        LOG.with(|l| l.borrow_mut().push(msg.to_string()));
    }

    fn logging_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        log_entry("handler");
        response_with_text("handler_response")
    }

    fn root_middleware(
        req: HttpRequest,
        params: &RouteParams,
        next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
    ) -> HttpResponse<'static> {
        log_entry("root_mw_before");
        let resp = next(req, params);
        log_entry("root_mw_after");
        resp
    }

    fn api_middleware(
        req: HttpRequest,
        params: &RouteParams,
        next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
    ) -> HttpResponse<'static> {
        log_entry("api_mw_before");
        let resp = next(req, params);
        log_entry("api_mw_after");
        resp
    }

    fn api_v2_middleware(
        req: HttpRequest,
        params: &RouteParams,
        next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
    ) -> HttpResponse<'static> {
        log_entry("api_v2_mw_before");
        let resp = next(req, params);
        log_entry("api_v2_mw_after");
        resp
    }

    /// 2.2.6a: Root middleware runs on all requests
    #[test]
    fn test_root_middleware_runs_on_all_requests() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/", Method::GET, logging_handler);
        root.insert("/about", Method::GET, logging_handler);
        root.insert("/api/users", Method::GET, logging_handler);
        root.set_middleware("/", root_middleware);

        // Root path
        let (handler, params) = resolve_get(&root, "/");
        root.execute_with_middleware("/", handler, test_request("/"), params);
        assert!(get_log().contains(&"root_mw_before".to_string()));
        assert!(get_log().contains(&"handler".to_string()));
        assert!(get_log().contains(&"root_mw_after".to_string()));

        // /about
        clear_log();
        let (handler, params) = resolve_get(&root, "/about");
        root.execute_with_middleware("/about", handler, test_request("/about"), params);
        assert!(get_log().contains(&"root_mw_before".to_string()));
        assert!(get_log().contains(&"handler".to_string()));

        // /api/users
        clear_log();
        let (handler, params) = resolve_get(&root, "/api/users");
        root.execute_with_middleware("/api/users", handler, test_request("/api/users"), params);
        assert!(get_log().contains(&"root_mw_before".to_string()));
        assert!(get_log().contains(&"handler".to_string()));
    }

    /// 2.2.6b: Scoped middleware runs only on matching prefix
    #[test]
    fn test_scoped_middleware_only_matching_prefix() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/users", Method::GET, logging_handler);
        root.insert("/pages/home", Method::GET, logging_handler);
        root.set_middleware("/api", api_middleware);

        // /api/users — api_middleware should run
        let (handler, params) = resolve_get(&root, "/api/users");
        root.execute_with_middleware("/api/users", handler, test_request("/api/users"), params);
        assert!(get_log().contains(&"api_mw_before".to_string()));
        assert!(get_log().contains(&"handler".to_string()));

        // /pages/home — api_middleware should NOT run
        clear_log();
        let (handler, params) = resolve_get(&root, "/pages/home");
        root.execute_with_middleware("/pages/home", handler, test_request("/pages/home"), params);
        assert!(!get_log().contains(&"api_mw_before".to_string()));
        assert!(get_log().contains(&"handler".to_string()));
    }

    /// 2.2.6c: Chain order is root → outer → inner → handler → inner → outer → root
    #[test]
    fn test_middleware_chain_order() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/v2/data", Method::GET, logging_handler);
        root.set_middleware("/", root_middleware);
        root.set_middleware("/api", api_middleware);
        root.set_middleware("/api/v2", api_v2_middleware);

        let (handler, params) = resolve_get(&root, "/api/v2/data");
        root.execute_with_middleware(
            "/api/v2/data",
            handler,
            test_request("/api/v2/data"),
            params,
        );

        let log = get_log();
        assert_eq!(
            log,
            vec![
                "root_mw_before",
                "api_mw_before",
                "api_v2_mw_before",
                "handler",
                "api_v2_mw_after",
                "api_mw_after",
                "root_mw_after",
            ]
        );
    }

    /// 2.2.6d: Middleware can short-circuit (return without calling next)
    #[test]
    fn test_middleware_short_circuit() {
        fn auth_middleware(
            _req: HttpRequest,
            _params: &RouteParams,
            _next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
        ) -> HttpResponse<'static> {
            log_entry("auth_reject");
            HttpResponse::builder()
                .with_status_code(StatusCode::UNAUTHORIZED)
                .with_body(Cow::Owned(b"Unauthorized".to_vec()))
                .build()
        }

        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/secret", Method::GET, logging_handler);
        root.set_middleware("/", auth_middleware);

        let (handler, params) = resolve_get(&root, "/secret");
        let resp =
            root.execute_with_middleware("/secret", handler, test_request("/secret"), params);

        assert_eq!(resp.status_code(), StatusCode::UNAUTHORIZED);
        let log = get_log();
        assert!(log.contains(&"auth_reject".to_string()));
        assert!(!log.contains(&"handler".to_string()));
    }

    /// 2.2.6e: Middleware can modify the response from next
    #[test]
    fn test_middleware_modifies_response() {
        fn header_middleware(
            req: HttpRequest,
            params: &RouteParams,
            next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
        ) -> HttpResponse<'static> {
            let resp = next(req, params);
            // Build a new response with an added header.
            let mut headers = resp.headers().to_vec();
            headers.push(("x-custom".to_string(), "injected".to_string()));
            HttpResponse::builder()
                .with_status_code(resp.status_code())
                .with_headers(headers)
                .with_body(resp.body().to_vec())
                .build()
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/test", Method::GET, logging_handler);
        root.set_middleware("/", header_middleware);

        let (handler, params) = resolve_get(&root, "/test");
        let resp = root.execute_with_middleware("/test", handler, test_request("/test"), params);

        let custom_header = resp
            .headers()
            .iter()
            .find(|(k, _)| k == "x-custom")
            .map(|(_, v)| v.clone());
        assert_eq!(custom_header, Some("injected".to_string()));
        assert_eq!(body_str(resp), "handler_response");
    }

    /// 2.2.6f: set_middleware on same prefix replaces previous middleware
    #[test]
    fn test_set_middleware_replaces_previous() {
        fn mw_a(
            req: HttpRequest,
            params: &RouteParams,
            next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
        ) -> HttpResponse<'static> {
            log_entry("mw_a");
            next(req, params)
        }
        fn mw_b(
            req: HttpRequest,
            params: &RouteParams,
            next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
        ) -> HttpResponse<'static> {
            log_entry("mw_b");
            next(req, params)
        }

        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/test", Method::GET, logging_handler);
        root.set_middleware("/", mw_a);
        root.set_middleware("/", mw_b); // should replace mw_a

        let (handler, params) = resolve_get(&root, "/test");
        root.execute_with_middleware("/test", handler, test_request("/test"), params);

        let log = get_log();
        assert!(!log.contains(&"mw_a".to_string()));
        assert!(log.contains(&"mw_b".to_string()));
    }

    /// 2.2.6g: Middleware works in both query and update paths.
    /// This tests that execute_with_middleware works correctly (same function
    /// is used by both http_request and http_request_update).
    #[test]
    fn test_middleware_works_in_both_paths() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));

        fn post_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            log_entry("post_handler");
            response_with_text("posted")
        }

        root.insert("/api/data", Method::GET, logging_handler);
        root.insert("/api/data", Method::POST, post_handler);
        root.set_middleware("/api", api_middleware);

        // Simulate query path (GET)
        let (handler, params) = resolve_get(&root, "/api/data");
        let resp =
            root.execute_with_middleware("/api/data", handler, test_request("/api/data"), params);
        assert_eq!(body_str(resp), "handler_response");
        assert!(get_log().contains(&"api_mw_before".to_string()));

        // Simulate update path (POST)
        clear_log();
        match root.resolve("/api/data", &Method::POST) {
            RouteResult::Found(handler, params, _) => {
                let req = HttpRequest::builder()
                    .with_method(Method::POST)
                    .with_url("/api/data")
                    .build();
                let resp = root.execute_with_middleware("/api/data", handler, req, params);
                assert_eq!(body_str(resp), "posted");
                assert!(get_log().contains(&"api_mw_before".to_string()));
                assert!(get_log().contains(&"post_handler".to_string()));
            }
            other => panic!("expected Found, got {}", route_result_name(&other)),
        }
    }

    // ---- 2.3 Custom 404 tests ----

    fn custom_404_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
        HttpResponse::builder()
            .with_status_code(StatusCode::NOT_FOUND)
            .with_headers(vec![("content-type".to_string(), "text/html".to_string())])
            .with_body(Cow::Owned(b"<h1>Custom Not Found</h1>".to_vec()))
            .build()
    }

    /// 2.3.4a: With custom 404, unmatched route returns custom response
    #[test]
    fn test_custom_404_returns_custom_response() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/exists", Method::GET, matched_get_handler);
        root.set_not_found(custom_404_handler);

        // Unmatched path should invoke the custom 404 handler
        let resp = root
            .execute_not_found_with_middleware("/nonexistent", test_request("/nonexistent"))
            .expect("expected custom 404 response");
        assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(body_str(resp), "<h1>Custom Not Found</h1>");
    }

    /// 2.3.4b: Without custom 404, unmatched route returns default "Not Found"
    #[test]
    fn test_default_404_without_custom_handler() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/exists", Method::GET, matched_get_handler);
        // No set_not_found call

        // execute_not_found_with_middleware should return None
        let resp =
            root.execute_not_found_with_middleware("/nonexistent", test_request("/nonexistent"));
        assert!(resp.is_none(), "expected None when no custom 404 is set");
    }

    /// 2.3.4c: Custom 404 handler receives the full HttpRequest
    #[test]
    fn test_custom_404_receives_full_request() {
        fn inspecting_404(req: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            // Echo back the URL from the request to prove it was passed through
            let url = req.url().to_string();
            response_with_text(&format!("404 for: {url}"))
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.set_not_found(inspecting_404);

        let req = HttpRequest::builder()
            .with_method(Method::GET)
            .with_url("/some/missing/path")
            .build();
        let resp = root
            .execute_not_found_with_middleware("/some/missing/path", req)
            .expect("expected custom 404 response");
        let body = body_str(resp);
        assert!(
            body.contains("/some/missing/path"),
            "expected URL in response body, got: {body}"
        );
    }

    /// 2.3.4d: Custom 404 can return JSON content-type
    #[test]
    fn test_custom_404_json_content_type() {
        fn json_404(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            HttpResponse::builder()
                .with_status_code(StatusCode::NOT_FOUND)
                .with_headers(vec![(
                    "content-type".to_string(),
                    "application/json".to_string(),
                )])
                .with_body(Cow::Owned(br#"{"error":"not found"}"#.to_vec()))
                .build()
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.set_not_found(json_404);

        let resp = root
            .execute_not_found_with_middleware("/api/missing", test_request("/api/missing"))
            .expect("expected custom 404 response");
        assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
        let ct = resp
            .headers()
            .iter()
            .find(|(k, _)| k == "content-type")
            .map(|(_, v)| v.clone());
        assert_eq!(ct, Some("application/json".to_string()));
        assert_eq!(body_str(resp), r#"{"error":"not found"}"#);
    }

    /// 2.3.4e: Root middleware executes before custom 404 handler
    #[test]
    fn test_root_middleware_runs_before_custom_404() {
        fn logging_404(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            log_entry("custom_404");
            response_with_text("custom 404")
        }

        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/exists", Method::GET, logging_handler);
        root.set_middleware("/", root_middleware);
        root.set_not_found(logging_404);

        let resp = root
            .execute_not_found_with_middleware("/nonexistent", test_request("/nonexistent"))
            .expect("expected custom 404 response");

        let log = get_log();
        assert_eq!(
            log,
            vec!["root_mw_before", "custom_404", "root_mw_after"],
            "middleware should wrap the custom 404 handler"
        );
        assert_eq!(body_str(resp), "custom 404");
    }

    // ---- 4.3.11: From<HttpResponse> for HandlerResult conversion ----

    #[test]
    fn from_http_response_for_handler_result() {
        let response = HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_body(Cow::Owned(b"hello".to_vec()))
            .build();

        let result: HandlerResult = response.into();

        match result {
            HandlerResult::Response(resp) => {
                assert_eq!(resp.status_code(), StatusCode::OK);
                assert_eq!(resp.body(), b"hello");
            }
            HandlerResult::NotModified => panic!("expected Response, got NotModified"),
        }
    }

    // ---- 5.5.2: Router edge case tests ----

    /// Empty segments in paths are ignored by the trie (split + filter removes them).
    #[test]
    fn test_empty_segments_ignored() {
        let root = setup_router();
        // Triple slash between segments should still resolve
        let (handler, _) = resolve_get(&root, "/about///");
        assert_eq!(
            body_str(handler(test_request("/about"), HashMap::new())),
            "about"
        );
    }

    /// URL-encoded characters are passed as-is to the trie (the trie does not decode).
    /// The handler receives the raw URL-encoded segment.
    #[test]
    fn test_url_encoded_characters_in_static_path() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        // Register a route with a literal percent-encoded segment.
        root.insert("/hello%20world", Method::GET, matched_about);
        let (handler, params) = resolve_get(&root, "/hello%20world");
        assert_eq!(
            body_str(handler(test_request("/hello%20world"), params)),
            "about"
        );
    }

    /// URL-encoded characters captured by a param route are preserved as-is.
    #[test]
    fn test_url_encoded_characters_in_param() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/posts/:id", Method::GET, matched_deep);
        let (_, params) = resolve_get(&root, "/posts/hello%20world");
        assert_eq!(params.get("id").unwrap(), "hello%20world");
    }

    /// Very long paths (100 segments) are handled without stack overflow or panic.
    #[test]
    fn test_very_long_path() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        // Build a path with 100 static segments
        let segments: Vec<String> = (0..100).map(|i| format!("s{i}")).collect();
        let path = format!("/{}", segments.join("/"));
        root.insert(&path, Method::GET, matched_about);

        let (handler, params) = resolve_get(&root, &path);
        assert_eq!(body_str(handler(test_request(&path), params)), "about");
    }

    /// Very long path that does not match any route returns NotFound.
    #[test]
    fn test_very_long_path_not_found() {
        let root = RouteNode::new(NodeType::Static("".into()));
        let segments: Vec<String> = (0..100).map(|i| format!("s{i}")).collect();
        let path = format!("/{}", segments.join("/"));
        assert!(matches!(
            root.resolve(&path, &Method::GET),
            RouteResult::NotFound
        ));
    }

    /// Routes with many (4) dynamic parameters all capture correctly.
    #[test]
    fn test_many_parameters() {
        fn many_param_handler(_: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
            response_with_text(&format!(
                "{}/{}/{}/{}",
                params.get("a").unwrap(),
                params.get("b").unwrap(),
                params.get("c").unwrap(),
                params.get("d").unwrap(),
            ))
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/:a/:b/:c/:d", Method::GET, many_param_handler);

        let (handler, params) = resolve_get(&root, "/w/x/y/z");
        assert_eq!(params.get("a").unwrap(), "w");
        assert_eq!(params.get("b").unwrap(), "x");
        assert_eq!(params.get("c").unwrap(), "y");
        assert_eq!(params.get("d").unwrap(), "z");
        assert_eq!(
            body_str(handler(test_request("/w/x/y/z"), params)),
            "w/x/y/z"
        );
    }

    /// Static route takes precedence over param route for the same segment.
    #[test]
    fn test_static_precedence_over_param() {
        fn static_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            response_with_text("static")
        }
        fn param_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            response_with_text("param")
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/items/special", Method::GET, static_handler);
        root.insert("/items/:id", Method::GET, param_handler);

        // "/items/special" should match the static route
        let (handler, _) = resolve_get(&root, "/items/special");
        assert_eq!(
            body_str(handler(test_request("/items/special"), HashMap::new())),
            "static"
        );

        // "/items/other" should match the param route
        let (handler, params) = resolve_get(&root, "/items/other");
        assert_eq!(
            body_str(handler(test_request("/items/other"), params)),
            "param"
        );
    }

    /// Param route takes precedence over wildcard route.
    #[test]
    fn test_param_precedence_over_wildcard() {
        fn param_handler(_: HttpRequest, params: RouteParams) -> HttpResponse<'static> {
            response_with_text(&format!("param:{}", params.get("id").unwrap()))
        }
        fn wildcard_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            response_with_text("wildcard")
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/items/:id", Method::GET, param_handler);
        root.insert("/items/*", Method::GET, wildcard_handler);

        // Single segment after /items/ should match param route
        let (handler, params) = resolve_get(&root, "/items/42");
        assert_eq!(
            body_str(handler(test_request("/items/42"), params.clone())),
            "param:42"
        );

        // Multiple segments should match wildcard
        let (handler, params) = resolve_get(&root, "/items/42/extra");
        assert_eq!(params.get("*").unwrap(), "42/extra");
        assert_eq!(
            body_str(handler(test_request("/items/42/extra"), params)),
            "wildcard"
        );
    }

    /// Root path "/" should not match when only nested routes exist.
    #[test]
    fn test_root_not_found_when_only_nested() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/data", Method::GET, matched_about);
        assert!(matches!(
            root.resolve("/", &Method::GET),
            RouteResult::NotFound
        ));
    }

    /// insert_result and resolve return the result handler.
    #[test]
    fn test_insert_result_and_resolve() {
        fn handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            response_with_text("ok")
        }
        fn result_handler(_: HttpRequest, _: RouteParams) -> HandlerResult {
            HandlerResult::NotModified
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/test", Method::GET, handler);
        root.insert_result("/test", Method::GET, result_handler);

        match root.resolve("/test", &Method::GET) {
            RouteResult::Found(_, _, Some(rh)) => {
                // Verify the result handler returns NotModified
                let result = rh(test_request("/test"), HashMap::new());
                assert!(matches!(result, HandlerResult::NotModified));
            }
            RouteResult::Found(_, _, None) => panic!("expected result handler to be present"),
            other => panic!("expected Found, got {}", route_result_name(&other)),
        }
    }

    /// match_path returns handlers and params without method dispatch.
    #[test]
    fn test_match_path_returns_handlers() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/items/:id", Method::GET, matched_get_handler);
        root.insert("/items/:id", Method::POST, matched_post_handler);

        let (handlers, _, params) = root.match_path("/items/42").expect("should match");
        assert_eq!(params.get("id").unwrap(), "42");
        assert!(handlers.contains_key(&Method::GET));
        assert!(handlers.contains_key(&Method::POST));
        assert_eq!(handlers.len(), 2);
    }

    /// match_path returns None for non-existent paths.
    #[test]
    fn test_match_path_returns_none() {
        let root = RouteNode::new(NodeType::Static("".into()));
        assert!(root.match_path("/nonexistent").is_none());
    }

    // ---- 5.5.3: Additional middleware chain tests ----

    /// Middleware can modify the request before passing it to the handler.
    /// The handler sees the modified request (e.g. added headers).
    #[test]
    fn test_middleware_modifies_request_before_handler() {
        fn inject_header_mw(
            req: HttpRequest,
            params: &RouteParams,
            next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
        ) -> HttpResponse<'static> {
            // Build a new request with an added header.
            let mut headers = req.headers().to_vec();
            headers.push(("x-injected".to_string(), "mw-value".to_string()));
            let modified = HttpRequest::builder()
                .with_method(req.method().clone())
                .with_url(req.url())
                .with_headers(headers)
                .with_body(req.body().to_vec())
                .build();
            next(modified, params)
        }

        fn header_checking_handler(req: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            let has_header = req
                .headers()
                .iter()
                .any(|(k, v)| k == "x-injected" && v == "mw-value");
            if has_header {
                response_with_text("header_present")
            } else {
                response_with_text("header_missing")
            }
        }

        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/check", Method::GET, header_checking_handler);
        root.set_middleware("/", inject_header_mw);

        let (handler, params) = resolve_get(&root, "/check");
        let resp = root.execute_with_middleware("/check", handler, test_request("/check"), params);
        assert_eq!(body_str(resp), "header_present");
    }

    /// Multiple middleware at different hierarchy levels all apply to not-found handler.
    #[test]
    fn test_multiple_middleware_on_not_found() {
        fn nf_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            log_entry("not_found_handler");
            response_with_text("not found")
        }

        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/data", Method::GET, logging_handler);
        root.set_middleware("/", root_middleware);
        root.set_middleware("/api", api_middleware);
        root.set_not_found(nf_handler);

        // Request to /api/missing — both root and /api middleware should fire
        let resp = root
            .execute_not_found_with_middleware("/api/missing", test_request("/api/missing"))
            .expect("expected not-found response");

        let log = get_log();
        assert_eq!(
            log,
            vec![
                "root_mw_before",
                "api_mw_before",
                "not_found_handler",
                "api_mw_after",
                "root_mw_after",
            ],
            "both root and /api middleware should wrap the not-found handler"
        );
        assert_eq!(body_str(resp), "not found");
    }

    /// Only root middleware applies to not-found for paths outside /api.
    #[test]
    fn test_not_found_only_root_middleware_for_non_api() {
        fn nf_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            log_entry("not_found_handler");
            response_with_text("not found")
        }

        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/data", Method::GET, logging_handler);
        root.set_middleware("/", root_middleware);
        root.set_middleware("/api", api_middleware);
        root.set_not_found(nf_handler);

        // Request to /other/missing — only root middleware, NOT /api middleware
        let resp = root
            .execute_not_found_with_middleware("/other/missing", test_request("/other/missing"))
            .expect("expected not-found response");

        let log = get_log();
        assert_eq!(
            log,
            vec!["root_mw_before", "not_found_handler", "root_mw_after"],
            "/api middleware should NOT fire for /other/missing"
        );
        assert_eq!(body_str(resp), "not found");
    }

    /// Middleware executes in correct order regardless of the registration order.
    /// (Ordering is by prefix segment count, not insertion order.)
    #[test]
    fn test_middleware_ordering_independent_of_registration_order() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/api/v2/data", Method::GET, logging_handler);

        // Register in reverse order: inner → outer → root
        root.set_middleware("/api/v2", api_v2_middleware);
        root.set_middleware("/api", api_middleware);
        root.set_middleware("/", root_middleware);

        let (handler, params) = resolve_get(&root, "/api/v2/data");
        root.execute_with_middleware(
            "/api/v2/data",
            handler,
            test_request("/api/v2/data"),
            params,
        );

        let log = get_log();
        assert_eq!(
            log,
            vec![
                "root_mw_before",
                "api_mw_before",
                "api_v2_mw_before",
                "handler",
                "api_v2_mw_after",
                "api_mw_after",
                "root_mw_after",
            ],
            "order should be root→api→api_v2 regardless of registration order"
        );
    }

    /// No middleware registered — handler runs directly without wrapping.
    #[test]
    fn test_no_middleware_handler_runs_directly() {
        clear_log();
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/test", Method::GET, logging_handler);
        // No set_middleware calls

        let (handler, params) = resolve_get(&root, "/test");
        let resp = root.execute_with_middleware("/test", handler, test_request("/test"), params);

        let log = get_log();
        assert_eq!(log, vec!["handler"]);
        assert_eq!(body_str(resp), "handler_response");
    }

    /// normalize_prefix normalizes various formats to canonical form.
    #[test]
    fn test_normalize_prefix_canonical() {
        assert_eq!(normalize_prefix("/"), "/");
        assert_eq!(normalize_prefix(""), "/");
        assert_eq!(normalize_prefix("/api"), "/api");
        assert_eq!(normalize_prefix("/api/"), "/api");
        assert_eq!(normalize_prefix("api"), "/api");
        assert_eq!(normalize_prefix("api/v2/"), "/api/v2");
    }

    /// segment_count returns correct counts.
    #[test]
    fn test_segment_count() {
        assert_eq!(segment_count("/"), 0);
        assert_eq!(segment_count("/api"), 1);
        assert_eq!(segment_count("/api/v2"), 2);
        assert_eq!(segment_count("/api/v2/data"), 3);
    }

    /// path_matches_prefix works for various combinations.
    #[test]
    fn test_path_matches_prefix() {
        // Root prefix matches everything
        assert!(path_matches_prefix("/api/data", "/"));
        assert!(path_matches_prefix("/", "/"));

        // Exact match
        assert!(path_matches_prefix("/api", "/api"));

        // Prefix match with separator
        assert!(path_matches_prefix("/api/data", "/api"));
        assert!(path_matches_prefix("/api/v2/data", "/api"));

        // Does not match partial segment
        assert!(!path_matches_prefix("/api-v2", "/api"));
        assert!(!path_matches_prefix("/apidata", "/api"));

        // No match
        assert!(!path_matches_prefix("/other", "/api"));
    }

    // ---- 5.5.7: Property-based tests (proptest) ----

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn dummy_handler(_: HttpRequest, _: RouteParams) -> HttpResponse<'static> {
            response_with_text("dummy")
        }

        proptest! {
            /// Inserted routes are always found: any valid path that is inserted
            /// should resolve to Found for the same method.
            #[test]
            fn inserted_routes_are_always_found(path in "/[a-z]{1,5}(/[a-z]{1,5}){0,4}") {
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert(&path, Method::GET, dummy_handler);
                match root.resolve(&path, &Method::GET) {
                    RouteResult::Found(_, _, _) => {},
                    _ => panic!("expected Found for inserted path: {path}"),
                }
            }

            /// Non-inserted routes are not found: a route that was never inserted
            /// should resolve to NotFound (assuming no wildcard or param overlap).
            #[test]
            fn non_inserted_routes_are_not_found(
                inserted in "/[a-z]{1,10}",
                queried in "/[a-z]{1,10}"
            ) {
                prop_assume!(inserted != queried);
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert(&inserted, Method::GET, dummy_handler);
                match root.resolve(&queried, &Method::GET) {
                    RouteResult::NotFound => {},
                    _ => panic!("expected NotFound for non-inserted route: {queried} (inserted: {inserted})"),
                }
            }

            /// Param routes capture any single segment value.
            #[test]
            fn param_routes_capture_any_segment(
                prefix in "/[a-z]{1,5}",
                value in "[a-z0-9]{1,20}"
            ) {
                let route = format!("{prefix}/:id");
                let path = format!("{prefix}/{value}");
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert(&route, Method::GET, dummy_handler);
                match root.resolve(&path, &Method::GET) {
                    RouteResult::Found(_, params, _) => {
                        prop_assert_eq!(params.get("id").map(|s| s.as_str()), Some(value.as_str()));
                    },
                    other => panic!("expected Found, got {}", route_result_name(&other)),
                }
            }

            /// Wildcard routes capture the remaining path (one or more segments).
            #[test]
            fn wildcard_routes_capture_remaining_path(
                prefix in "/[a-z]{1,5}",
                tail in "[a-z0-9]{1,5}(/[a-z0-9]{1,5}){0,3}"
            ) {
                let route = format!("{prefix}/*");
                let path = format!("{prefix}/{tail}");
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert(&route, Method::GET, dummy_handler);
                match root.resolve(&path, &Method::GET) {
                    RouteResult::Found(_, params, _) => {
                        prop_assert_eq!(params.get("*").map(|s| s.as_str()), Some(tail.as_str()));
                    },
                    other => panic!("expected Found, got {}", route_result_name(&other)),
                }
            }

            /// Inserting a route does not affect resolution of a different method
            /// on the same path — it should return MethodNotAllowed, not Found.
            #[test]
            fn wrong_method_returns_method_not_allowed(path in "/[a-z]{1,5}(/[a-z]{1,5}){0,3}") {
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert(&path, Method::GET, dummy_handler);
                match root.resolve(&path, &Method::POST) {
                    RouteResult::MethodNotAllowed(allowed) => {
                        prop_assert!(allowed.contains(&Method::GET));
                    },
                    other => panic!("expected MethodNotAllowed, got {}", route_result_name(&other)),
                }
            }

            /// Multiple param routes with different names capture correctly.
            #[test]
            fn multi_param_routes_capture_all(
                a in "[a-z0-9]{1,10}",
                b in "[a-z0-9]{1,10}"
            ) {
                let mut root = RouteNode::new(NodeType::Static("".into()));
                root.insert("/x/:first/:second", Method::GET, dummy_handler);
                let path = format!("/x/{a}/{b}");
                match root.resolve(&path, &Method::GET) {
                    RouteResult::Found(_, params, _) => {
                        prop_assert_eq!(params.get("first").map(|s| s.as_str()), Some(a.as_str()));
                        prop_assert_eq!(params.get("second").map(|s| s.as_str()), Some(b.as_str()));
                    },
                    other => panic!("expected Found, got {}", route_result_name(&other)),
                }
            }
        }
    }
}

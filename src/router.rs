// Updated router with HttpRequest passed into handler
use ic_http_certification::{HttpRequest, HttpResponse, Method};
use std::collections::HashMap;

use crate::middleware::MiddlewareFn;

pub type RouteParams = HashMap<String, String>;
pub type HandlerFn = fn(HttpRequest, RouteParams) -> HttpResponse<'static>;

#[derive(Debug, PartialEq, Eq)]
pub enum NodeType {
    Static(String),
    Param(String),
    Wildcard,
}

/// Result of resolving a path + method against the route tree.
pub enum RouteResult {
    /// A handler was found for the given path and method.
    Found(HandlerFn, RouteParams),
    /// The path exists but the requested method is not registered.
    /// Contains the list of methods that *are* registered for this path.
    MethodNotAllowed(Vec<Method>),
    /// No route matches the given path.
    NotFound,
}

pub struct RouteNode {
    pub node_type: NodeType,
    pub children: Vec<RouteNode>,
    pub handlers: HashMap<Method, HandlerFn>,
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
    pub fn new(node_type: NodeType) -> Self {
        Self {
            node_type,
            children: Vec::new(),
            handlers: HashMap::new(),
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

    pub fn insert(&mut self, path: &str, method: Method, handler: HandlerFn) {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._insert(&segments, method, handler);
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
            Some((handlers, params)) => {
                if let Some(&handler) = handlers.get(method) {
                    RouteResult::Found(handler, params)
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
    pub fn match_path(&self, path: &str) -> Option<(&HashMap<Method, HandlerFn>, RouteParams)> {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._match(&segments)
    }

    fn _match(&self, segments: &[&str]) -> Option<(&HashMap<Method, HandlerFn>, RouteParams)> {
        if segments.is_empty() {
            if !self.handlers.is_empty() {
                return Some((&self.handlers, HashMap::new()));
            }
            // No handlers on this node — check for a wildcard child (empty wildcard match)
            for child in &self.children {
                if let NodeType::Wildcard = child.node_type {
                    if !child.handlers.is_empty() {
                        let mut params = HashMap::new();
                        params.insert("*".to_string(), String::new());
                        return Some((&child.handlers, params));
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
                    if let Some((h, p)) = child._match(tail) {
                        debug_log!("Static match: {:?}", segments);
                        return Some((h, p));
                    }
                }
            }
        }

        // Param match
        for child in &self.children {
            if let NodeType::Param(ref name) = child.node_type {
                if let Some((h, mut p)) = child._match(tail) {
                    p.insert(name.clone(), head.to_string());
                    debug_log!("Param match: {:?}", segments);
                    return Some((h, p));
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
                    return Some((&child.handlers, params));
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

#[cfg(test)]
mod tests {
    use super::*;
    use ic_http_certification::{Method, StatusCode};
    use std::{borrow::Cow, str};

    fn test_request(path: &str) -> HttpRequest {
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
            RouteResult::Found(h, p) => (h, p),
            other => panic!(
                "expected Found for GET {path}, got {}",
                route_result_name(&other)
            ),
        }
    }

    fn route_result_name(r: &RouteResult) -> &'static str {
        match r {
            RouteResult::Found(_, _) => "Found",
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
            RouteResult::Found(handler, params) => {
                assert_eq!(
                    body_str(handler(test_request("/api/users"), params)),
                    "get_handler"
                );
            }
            other => panic!("expected Found, got {}", route_result_name(&other)),
        }

        // POST resolves to post handler
        match root.resolve("/api/users", &Method::POST) {
            RouteResult::Found(handler, params) => {
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
                RouteResult::Found(_, _) => {}
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
            RouteResult::Found(handler, params) => {
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
}

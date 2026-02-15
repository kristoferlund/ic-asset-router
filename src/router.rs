// Updated router with HttpRequest passed into handler
use ic_http_certification::{HttpRequest, HttpResponse, Method};
use std::collections::HashMap;

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
}

impl RouteNode {
    pub fn new(node_type: NodeType) -> Self {
        Self {
            node_type,
            children: Vec::new(),
            handlers: HashMap::new(),
        }
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
}

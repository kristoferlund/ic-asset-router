// Updated router with HttpRequest passed into handler
use ic_http_certification::{HttpRequest, HttpResponse};
use std::collections::HashMap;

pub type RouteParams = HashMap<String, String>;
pub type HandlerFn = fn(HttpRequest, RouteParams) -> HttpResponse<'static>;

#[derive(Debug, PartialEq, Eq)]
pub enum NodeType {
    Static(String),
    Param(String),
    Wildcard,
}

pub struct RouteNode {
    pub node_type: NodeType,
    pub children: Vec<RouteNode>,
    pub handler: Option<HandlerFn>,
}

impl RouteNode {
    pub fn new(node_type: NodeType) -> Self {
        Self {
            node_type,
            children: Vec::new(),
            handler: None,
        }
    }

    pub fn insert(&mut self, path: &str, handler: HandlerFn) {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._insert(&segments, handler);
    }

    fn _insert(&mut self, segments: &[&str], handler: HandlerFn) {
        if segments.is_empty() {
            self.handler = Some(handler);
            return;
        }

        let node_type = match segments[0] {
            "*" => NodeType::Wildcard,
            s if s.starts_with(':') => NodeType::Param(s[1..].to_string()),
            s => NodeType::Static(s.to_string()),
        };

        let child = self.children.iter_mut().find(|c| c.node_type == node_type);

        match child {
            Some(c) => c._insert(&segments[1..], handler),
            None => {
                let mut new_node = RouteNode::new(node_type);
                new_node._insert(&segments[1..], handler);
                self.children.push(new_node);
            }
        }
    }

    pub fn match_path(&self, path: &str) -> Option<(HandlerFn, RouteParams)> {
        let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
        self._match(&segments)
    }

    fn _match(&self, segments: &[&str]) -> Option<(HandlerFn, RouteParams)> {
        if segments.is_empty() {
            if let Some(h) = self.handler {
                return Some((h, HashMap::new()));
            }
            // No handler on this node — check for a wildcard child (empty wildcard match)
            for child in &self.children {
                if let NodeType::Wildcard = child.node_type {
                    if let Some(h) = child.handler {
                        let mut params = HashMap::new();
                        params.insert("*".to_string(), String::new());
                        return Some((h, params));
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
                if !segments.is_empty() {
                    debug_log!("Wildcard match: {:?}", segments);
                    let remaining = segments.join("/");
                    let mut params = HashMap::new();
                    params.insert("*".to_string(), remaining);
                    return child.handler.map(|h| (h, params));
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
        root.insert("/", matched_root);
        root.insert("/*", matched_404);
        root.insert("/index2", matched_index2);
        root.insert("/about", matched_about);
        root.insert("/deep/:pageId", matched_deep);
        root.insert("/deep/:pageId/:subpageId", matched_deep);
        root.insert("/alsodeep/:pageId/edit", matched_deep);
        root.insert("/folder/*", matched_folder);
        root
    }

    fn body_str(resp: HttpResponse<'static>) -> String {
        str::from_utf8(resp.body())
            .unwrap_or("<invalid utf-8>")
            .to_string()
    }

    #[test]
    fn test_root_match() {
        let root = setup_router();
        let (handler, params) = root.match_path("/").unwrap();
        assert_eq!(body_str(handler(test_request("/"), params)), "root");
    }

    #[test]
    fn test_404_match() {
        let root = setup_router();
        let (handler, _) = root.match_path("/nonexistent").unwrap();
        assert_eq!(
            body_str(handler(test_request("/nonexistent"), HashMap::new())),
            "404"
        );
    }

    #[test]
    fn test_exact_match() {
        let root = setup_router();
        let (handler, params) = root.match_path("/index2").unwrap();
        assert_eq!(body_str(handler(test_request("/index2"), params)), "index2");
    }

    #[test]
    fn test_pathless_layout_route_a() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/about", matched_about);
        let (handler, params) = root.match_path("/about").unwrap();
        assert_eq!(body_str(handler(test_request("/about"), params)), "about");
    }

    #[test]
    fn test_dynamic_match() {
        let root = setup_router();
        let (handler, params) = root.match_path("/deep/page1").unwrap();
        let body = body_str(handler(test_request("/deep/page1"), params));
        assert!(body.contains("page1"));
    }

    #[test]
    fn test_posts_postid_edit() {
        let root = setup_router();
        let (handler, params) = root.match_path("/alsodeep/page1/edit").unwrap();
        let body = body_str(handler(test_request("/alsodeep/page1/edit"), params));
        assert!(body.contains("page1"));
    }

    #[test]
    fn test_nested_dynamic_match() {
        let root = setup_router();
        let (handler, params) = root.match_path("/deep/page2/subpage1").unwrap();
        let body = body_str(handler(test_request("/deep/page2/subpage1"), params));
        assert!(body.contains("page2"));
        assert!(body.contains("subpage1"));
    }

    #[test]
    fn test_wildcard_match() {
        let root = setup_router();
        let (handler, _) = root.match_path("/folder/anything").unwrap();
        assert_eq!(
            body_str(handler(test_request("/folder/anything"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_folder_root_wildcard_match() {
        let root = setup_router();
        let (handler, _) = root.match_path("/folder/any").unwrap();
        assert_eq!(
            body_str(handler(test_request("/folder/any"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_deep_wildcard_multi_segments() {
        let root = setup_router();
        let (handler, _) = root.match_path("/folder/a/b/c/d").unwrap();
        assert_eq!(
            body_str(handler(test_request("/folder/a/b/c/d"), HashMap::new())),
            "folder"
        );
    }

    #[test]
    fn test_trailing_slash_static_match() {
        let root = setup_router();
        let (handler, _) = root.match_path("/index2/").unwrap();
        assert_eq!(
            body_str(handler(test_request("/index2/"), HashMap::new())),
            "index2"
        );
    }

    #[test]
    fn test_double_slash_matches_normalized() {
        let root = setup_router();
        let (handler, _) = root.match_path("//index2").unwrap();
        assert_eq!(
            body_str(handler(test_request("//index2"), HashMap::new())),
            "index2"
        );
    }

    #[test]
    fn test_root_wildcard_captures_full_path() {
        let root = setup_router();
        let (_, params) = root.match_path("/a/b/c").unwrap();
        assert_eq!(params.get("*").unwrap(), "a/b/c");
    }

    #[test]
    fn test_folder_wildcard_captures_tail() {
        let root = setup_router();
        let (handler, params) = root.match_path("/folder/docs/report.pdf").unwrap();
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
        root.insert("/users/:id/files/*", matched_user_files);
        let (_, params) = root.match_path("/users/42/files/docs/report.pdf").unwrap();
        assert_eq!(params.get("id").unwrap(), "42");
        assert_eq!(params.get("*").unwrap(), "docs/report.pdf");
    }

    #[test]
    fn test_empty_wildcard_match() {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("/files/*", matched_folder);
        let (handler, params) = root.match_path("/files/").unwrap();
        assert_eq!(params.get("*").unwrap(), "");
        assert_eq!(
            body_str(handler(test_request("/files/"), params.clone())),
            "folder"
        );
    }
}

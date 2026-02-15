use crate::routes;
use ic_http_certification::Method;
use router_library::router::{NodeType, RouteNode};

thread_local! {
    pub static ROUTES: RouteNode = {
        let mut root = RouteNode::new(NodeType::Static("".into()));
        root.insert("", Method::GET, routes::index::get);
        root.insert("/posts/:postId/", Method::GET, routes::posts::postId::index::get);
        root.insert("/posts/:postId/comments", Method::GET, routes::posts::postId::comments::get);
        root
    };
}

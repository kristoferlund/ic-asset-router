use std::borrow::Cow;

use askama::Template;
use ic_http_certification::{HttpResponse, StatusCode};
use router_library::RouteContext;

use crate::data::{self, Comment};

use super::Params;

/// HTML fragment — no layout wrapper. Returned as an HTMX partial response.
#[derive(Template)]
#[template(path = "partials/comments.html")]
struct CommentsTemplate {
    comments: Vec<Comment>,
    post_id: String,
}

pub fn get(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = &ctx.params.post_id;
    let comments = data::comments_for_post(post_id);
    let template = CommentsTemplate {
        comments,
        post_id: post_id.to_string(),
    };

    render_template(&template)
}

pub fn post(ctx: RouteContext<Params>) -> HttpResponse<'static> {
    let post_id = ctx.params.post_id.clone();

    let body_str = std::str::from_utf8(&ctx.body).unwrap_or("");
    let fields = parse_form_urlencoded(body_str);

    let author = fields
        .get("author")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| "Anonymous".to_string());
    let body = fields.get("body").cloned().unwrap_or_default();

    if body.is_empty() {
        let comments = data::comments_for_post(&post_id);
        let template = CommentsTemplate { comments, post_id };
        return render_template(&template);
    }

    let comments = data::add_comment(&post_id, author, body);
    let template = CommentsTemplate { comments, post_id };
    render_template(&template)
}

fn render_template(template: &CommentsTemplate) -> HttpResponse<'static> {
    match template.render() {
        Ok(html) => HttpResponse::builder()
            .with_status_code(StatusCode::OK)
            .with_headers(vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )])
            .with_body(Cow::<[u8]>::Owned(html.into_bytes()))
            .build(),
        Err(_) => HttpResponse::builder()
            .with_status_code(StatusCode::INTERNAL_SERVER_ERROR)
            .with_headers(vec![("content-type".to_string(), "text/plain".to_string())])
            .with_body(Cow::<[u8]>::Owned(b"Template rendering failed".to_vec()))
            .build(),
    }
}

fn parse_form_urlencoded(input: &str) -> std::collections::HashMap<String, String> {
    input
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((url_decode(key), url_decode(value)))
        })
        .collect()
}

fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(result).unwrap_or_default()
}

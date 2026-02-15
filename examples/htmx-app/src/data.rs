use std::cell::RefCell;
use std::collections::HashMap;

/// A blog post.
pub struct Post {
    pub id: &'static str,
    pub title: &'static str,
    pub content: &'static str,
    pub author: &'static str,
}

/// A comment on a post.
#[derive(Clone)]
pub struct Comment {
    pub author: String,
    pub body: String,
}

thread_local! {
    static COMMENTS: RefCell<HashMap<String, Vec<Comment>>> = RefCell::new(seed_comments());
}

fn seed_comments() -> HashMap<String, Vec<Comment>> {
    let mut m = HashMap::new();
    m.insert(
        "1".into(),
        vec![
            Comment {
                author: "Bob".into(),
                body: "Great introduction! The HTTP gateway integration is really impressive."
                    .into(),
            },
            Comment {
                author: "Carol".into(),
                body: "Would love to see a follow-up on canister upgrades and stable memory."
                    .into(),
            },
        ],
    );
    m.insert(
        "2".into(),
        vec![
            Comment {
                author: "Alice".into(),
                body: "HTMX is such a refreshing approach. No build step, no bundler, just HTML."
                    .into(),
            },
            Comment {
                author: "Carol".into(),
                body: "The partial response pattern is perfect for canisters — small payloads, fast updates.".into(),
            },
            Comment {
                author: "Dave".into(),
                body: "How does this compare to using a frontend framework with canister APIs?"
                    .into(),
            },
        ],
    );
    m.insert(
        "3".into(),
        vec![Comment {
            author: "Alice".into(),
            body: "Askama is my go-to for canister projects. Compile-time errors save so much debugging.".into(),
        }],
    );
    m
}

/// All posts in the example dataset.
pub fn all_posts() -> &'static [Post] {
    &POSTS
}

/// Look up a single post by its id.
pub fn find_post(id: &str) -> Option<&'static Post> {
    POSTS.iter().find(|p| p.id == id)
}

/// Look up comments for a given post id.
pub fn comments_for_post(post_id: &str) -> Vec<Comment> {
    COMMENTS.with(|c| c.borrow().get(post_id).cloned().unwrap_or_default())
}

/// Add a comment to a post. Returns the updated comment list.
pub fn add_comment(post_id: &str, author: String, body: String) -> Vec<Comment> {
    COMMENTS.with(|c| {
        let mut map = c.borrow_mut();
        let comments = map.entry(post_id.to_string()).or_default();
        comments.push(Comment { author, body });
        comments.clone()
    })
}

static POSTS: [Post; 3] = [
    Post {
        id: "1",
        title: "Getting Started with ICP Canisters",
        content: "Internet Computer canisters are WebAssembly modules that run on-chain. \
                  They can serve HTTP responses directly, making them a natural fit for \
                  server-side rendered web applications.",
        author: "Alice",
    },
    Post {
        id: "2",
        title: "Server-Side Rendering with HTMX",
        content: "HTMX lets you build modern, interactive web applications using HTML \
                  attributes instead of JavaScript. Combined with server-side rendering, \
                  it brings back the simplicity of traditional web development while \
                  delivering a smooth user experience.",
        author: "Bob",
    },
    Post {
        id: "3",
        title: "Template Engines for Canisters",
        content: "Askama compiles templates at build time, producing zero-overhead rendering \
                  code. This is ideal for ICP canisters where binary size and execution \
                  speed directly affect cycle costs.",
        author: "Carol",
    },
];

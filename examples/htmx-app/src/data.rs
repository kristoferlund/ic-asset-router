/// A blog post.
pub struct Post {
    pub id: &'static str,
    pub title: &'static str,
    pub content: &'static str,
    pub author: &'static str,
}

/// A comment on a post.
pub struct Comment {
    pub author: &'static str,
    pub body: &'static str,
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
pub fn comments_for_post(post_id: &str) -> &'static [Comment] {
    match post_id {
        "1" => &COMMENTS_POST_1,
        "2" => &COMMENTS_POST_2,
        "3" => &COMMENTS_POST_3,
        _ => &[],
    }
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

static COMMENTS_POST_1: [Comment; 2] = [
    Comment {
        author: "Bob",
        body: "Great introduction! The HTTP gateway integration is really impressive.",
    },
    Comment {
        author: "Carol",
        body: "Would love to see a follow-up on canister upgrades and stable memory.",
    },
];

static COMMENTS_POST_2: [Comment; 3] = [
    Comment {
        author: "Alice",
        body: "HTMX is such a refreshing approach. No build step, no bundler, just HTML.",
    },
    Comment {
        author: "Carol",
        body:
            "The partial response pattern is perfect for canisters — small payloads, fast updates.",
    },
    Comment {
        author: "Dave",
        body: "How does this compare to using a frontend framework with canister APIs?",
    },
];

static COMMENTS_POST_3: [Comment; 1] = [Comment {
    author: "Alice",
    body: "Askama is my go-to for canister projects. Compile-time errors save so much debugging.",
}];

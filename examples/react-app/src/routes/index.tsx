import { createFileRoute, Link } from "@tanstack/react-router";
import useListPosts from "@/hooks/use-list-posts";

export const Route = createFileRoute("/")({
  component: Index,
});

function Index() {
  const { data: posts, isLoading, error } = useListPosts();

  return (
    <div>
      <h1>Blog Posts</h1>
      <p style={{ color: "#666", marginBottom: "2rem" }}>
        A simple React app running on the Internet Computer, demonstrating
        per-route SEO meta tags via ic-asset-router.
      </p>

      {isLoading && <p>Loading posts...</p>}
      {error && <p style={{ color: "red" }}>Failed to load posts.</p>}

      {posts && (
        <ul style={{ listStyle: "none", display: "flex", flexDirection: "column", gap: "1rem" }}>
          {posts.map((post) => (
            <li
              key={Number(post.id)}
              style={{
                border: "1px solid #e0e0e0",
                borderRadius: 8,
                padding: "1rem 1.25rem",
                background: "#fff",
              }}
            >
              <Link
                to="/posts/$postId"
                params={{ postId: String(post.id) }}
                style={{ fontSize: "1.125rem", fontWeight: 600 }}
              >
                {post.title}
              </Link>
              <p style={{ color: "#666", margin: "0.25rem 0 0" }}>
                {post.summary}
              </p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

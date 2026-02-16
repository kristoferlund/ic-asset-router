import { createFileRoute, Link } from "@tanstack/react-router";
import useGetPost from "@/hooks/use-get-post";

export const Route = createFileRoute("/posts/$postId")({
  component: PostDetail,
});

function PostDetail() {
  const { postId } = Route.useParams();
  const { data: post, isLoading, error } = useGetPost(Number(postId));

  if (isLoading) {
    return <p>Loading...</p>;
  }

  if (error || !post) {
    return (
      <div>
        <Link to="/">&larr; Back</Link>
        <p style={{ color: "#666", marginTop: "1rem" }}>
          {error ? "Failed to load post." : "Post not found."}
        </p>
      </div>
    );
  }

  return (
    <article>
      <Link to="/">&larr; Back</Link>
      <h1 style={{ marginTop: "1rem" }}>{post.title}</h1>
      <p style={{ color: "#666", fontStyle: "italic" }}>By {post.author}</p>
      <div
        style={{
          marginTop: "1.5rem",
          lineHeight: 1.8,
          whiteSpace: "pre-line",
        }}
      >
        {post.body}
      </div>
    </article>
  );
}

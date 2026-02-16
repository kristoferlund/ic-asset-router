import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/$")({
  component: NotFound,
});

function NotFound() {
  return (
    <div style={{ textAlign: "center", paddingTop: "4rem" }}>
      <h1 style={{ fontSize: "4rem", margin: 0, color: "#1a1a1a" }}>404</h1>
      <p style={{ color: "#666", fontSize: "1.125rem", marginTop: "0.5rem" }}>
        This page could not be found.
      </p>
      <Link
        to="/"
        style={{
          display: "inline-block",
          marginTop: "1.5rem",
          padding: "0.5rem 1.25rem",
          border: "1px solid #e0e0e0",
          borderRadius: 8,
          color: "#0066cc",
          background: "#fff",
        }}
      >
        Back to posts
      </Link>
    </div>
  );
}

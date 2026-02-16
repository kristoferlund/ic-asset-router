# react-app

React SPA with TanStack Router and TanStack Query, running entirely in an ICP canister.

Uses `ic-asset-router` for file-based routing and certified asset serving, `@icp-sdk/bindgen` for type-safe canister calls, and Tera for injecting per-route SEO meta tags into the Vite-built `index.html` at request time.

## Features

- Landing page listing posts fetched from the canister via TanStack Query
- Detail pages with post-specific `<title>`, `og:title`, and `og:description` meta tags
- Custom 404 page (server-side Tera rendering + client-side React catch-all route)
- Vite-built static assets (JS, CSS) served as certified query responses
- File-based routing on both frontend (TanStack Router) and backend (ic-asset-router)

## Run

```
pnpm install
dfx start --background
dfx deploy
```

Open the URL printed by `dfx deploy` in a browser.

For frontend development with hot reload:

```
pnpm run dev
```

## How per-route meta tags work

1. `index.html` contains Tera template tags (`{{ title }}`, `{{ description }}`) in its `<meta>` elements.
2. Vite preserves these tags during the production build.
3. On `init`, the canister certifies all static assets from `dist/`, then deletes the static `index.html` from the certification cache.
4. When a page route is requested, the corresponding Rust handler renders `dist/index.html` via Tera with route-specific values (e.g. the post title and summary for detail pages).
5. The rendered HTML is certified and cached by `ic-asset-router`.

## Project structure

```
index.html                Vite entry point with Tera template tags in meta elements
vite.config.ts            Vite plugins: TanStack Router, React SWC, ICP bindgen
src/
  main.tsx                React entry point (Router + Query providers)
  routes/
    __root.tsx            Root layout
    index.tsx             GET / — post listing via useListPosts()
    posts/$postId.tsx     GET /posts/:postId — post detail via useGetPost()
    $.tsx                 Catch-all 404 page
  hooks/
    use-server.ts         Singleton ICP agent and actor creation
    use-list-posts.ts     TanStack Query hook calling list_posts()
    use-get-post.ts       TanStack Query hook calling get_post()
server/
  server.did              Candid interface (list_posts, get_post, http_request)
  build.rs                Route tree generation
  src/
    lib.rs                Canister lifecycle, asset certification, Candid API
    data.rs               Hardcoded post data
    routes/
      index.rs            GET / — renders SPA shell with generic meta tags
      not_found.rs        404 — renders SPA shell with "not found" meta tags
      posts/_postId/
        index.rs          GET /posts/:postId — renders SPA shell with post meta tags
```

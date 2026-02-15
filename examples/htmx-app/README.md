# htmx-app

Server-side rendered blog with HTMX, running entirely in an ICP canister.

Uses `router_library` for file-based routing and certified asset serving, Askama for compile-time HTML templates, and HTMX for partial page updates without client-side JavaScript.

## Features

- Post listing and detail pages
- Lazy-loaded comments via HTMX partials
- Add comments via an inline form (POST handled in an update call)
- Static assets (CSS, JS) served as certified query responses

## Run

```
dfx start --background
dfx deploy
```

Open the URL printed by `dfx deploy` in a browser.

## Project structure

```
src/
  lib.rs              Canister entry points (init, http_request, http_request_update)
  data.rs             In-memory post and comment storage
  routes/
    index.rs          GET /
    posts/:postId/
      index.rs        GET /posts/:postId/
      comments.rs     GET & POST /posts/:postId/comments
templates/            Askama HTML templates
static/               CSS and JS served as certified static assets
build.rs              Generates the route tree from the file layout
```

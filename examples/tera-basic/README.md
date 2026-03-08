# tera-basic

Minimal canister demonstrating server-side HTML rendering with [Tera](https://keats.github.io/tera/) templates.

## Features demonstrated

- Runtime HTML templates via Tera (templates embedded at compile time via `include_str!`)
- Post listing and detail pages
- File-based routing with `ic-asset-router`

## Routes

| Path | Description |
|------|-------------|
| `/` | Home page listing posts |
| `/posts/:postId` | Post detail page |

## Project structure

```
src/
  lib.rs              Canister entry points
  routes/
    index.rs          GET /
    posts/
      _postId/
        index.rs      GET /posts/:postId
templates/
  index.html          Tera template for listing page
  post.html           Tera template for post detail
build.rs              Route tree generation
```

## Run

```
icp network start -d
icp deploy
```

Open the URL printed by `icp deploy` in a browser.

Stop the network when you're done:

```
icp network stop
```

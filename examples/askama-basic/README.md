# askama-basic

Minimal canister demonstrating server-side HTML rendering with [Askama](https://github.com/djc/askama) compile-time templates.

## Features demonstrated

- Compile-time HTML templates via Askama
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
      index.rs        GET /posts/:postId
templates/
  index.html          Askama template for listing page
  post.html           Askama template for post detail
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

# certification-modes

Canister demonstrating the three IC HTTP certification modes available in `ic-asset-router`.

## Features demonstrated

- **Response-only** (default): certifies the response body, status, and headers — no request data included
- **Skip**: no certification; suitable for monitoring endpoints where tampering has no security impact
- **Authenticated**: full certification including the `Authorization` request header
- **Custom**: full certification with specific query parameters (`page`, `limit`)

## Routes

| Path | Mode | Description |
|------|------|-------------|
| `/` | Response-only (default) | Overview page with links |
| `/public/health` | Skip | Health check — no certification needed |
| `/api/user` | Authenticated | Per-caller certified response (bound to `Authorization` header) |
| `/content/articles` | Custom | Paginated articles — certified per `page` and `limit` query params |

## When to use which mode

**Response-only** (no `#[route]` attribute): response is the same for all callers and does not depend on request headers or query parameters.

**Skip** (`#[route(certification = "skip")]`): real-time or monitoring data where cryptographic proof adds no value.

**Authenticated** (`#[route(certification = "authenticated")]`): response depends on the caller identity; prevents a replica from serving one user's data to another.

**Custom** (`#[route(certification = custom(query_params = [...]))]`): response depends on specific query parameters; certifies each unique parameter combination independently.

## Project structure

```
src/
  lib.rs                    Canister entry points
  routes/
    index.rs                GET / — response-only (default)
    public/
      health.rs             GET /public/health — skip
    api/
      user.rs               GET /api/user — authenticated
    content/
      articles.rs           GET /content/articles — custom (query params)
build.rs                    Route tree generation
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

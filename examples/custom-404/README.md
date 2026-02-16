# custom-404

Minimal canister demonstrating a custom `not_found.rs` handler that returns
styled HTML instead of the default plain-text 404 response.

## Features demonstrated

- Custom `not_found.rs` handler
- Styled HTML 404 page that shows the requested path
- The build script auto-detects `not_found.rs` and registers it

## Routes

| Path | Description |
|------|-------------|
| `/` | Home page with a link to a non-existent path |
| `/*` | Any unmatched path returns the styled 404 page |

## How it works

Place a `not_found.rs` file in the routes directory with a `pub fn get(...)`
export. The build script detects it and registers it as the custom 404 handler.
Any request that does not match a registered route invokes this handler instead
of the default "Not Found" response.

The handler receives a full `RouteContext<()>`, so it can inspect the URL,
headers, method, etc.

```rust
// src/routes/not_found.rs
pub fn get(ctx: RouteContext<()>) -> HttpResponse<'static> {
    let path = &ctx.url;
    // ... return styled HTML with the path
}
```

## Run

```
dfx start --background
dfx deploy
```

Visit any non-existent path (e.g. `/nope`) to see the custom 404 page.

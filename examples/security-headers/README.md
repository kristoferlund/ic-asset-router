# security-headers

Minimal canister demonstrating `SecurityHeaders` configuration.

## Features demonstrated

- `SecurityHeaders::strict()` — the most restrictive preset
- Comparison with `SecurityHeaders::permissive()` and custom configurations
- The index page fetches its own response headers and displays them

## Routes

| Path | Description |
|------|-------------|
| `/` | Displays response security headers (live) |
| `/permissive` | Documents the permissive preset for comparison |
| `/custom` | Shows how to build a custom header configuration |

## Configuration

In `src/lib.rs`, the canister is configured with:

```rust
ic_asset_router::set_asset_config(ic_asset_router::AssetConfig {
    security_headers: ic_asset_router::SecurityHeaders::strict(),
    ..ic_asset_router::AssetConfig::default()
});
```

Change `strict()` to `permissive()`, `none()`, or a custom struct to see
different header sets.

## Run

```
dfx start --background
dfx deploy
```

Open the URL printed by `dfx deploy` in a browser.

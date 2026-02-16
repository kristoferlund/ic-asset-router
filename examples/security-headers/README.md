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
router_library::set_asset_config(router_library::AssetConfig {
    security_headers: router_library::SecurityHeaders::strict(),
    ..router_library::AssetConfig::default()
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

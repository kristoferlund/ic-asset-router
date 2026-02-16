# cache-invalidation

Minimal canister demonstrating TTL-based cache expiry and explicit invalidation.

## Features demonstrated

- Default TTL (5 minutes) applied to all dynamic routes
- Per-route TTL override (30 seconds for `/ttl`)
- Explicit invalidation via `invalidate(path)` and `invalidate_all()` update calls
- `CacheConfig` with `default_ttl` and `per_route_ttl`

## Routes

| Path | Description |
|------|-------------|
| `/` | Returns the server timestamp; cached for 5 minutes |
| `/ttl` | Returns the server timestamp; cached for 30 seconds |

## Invalidation

After deploying, the first request to `/` triggers an update call that
generates and certifies the response. Subsequent requests serve the cached
version until either:

1. The TTL expires (5 min for `/`, 30 sec for `/ttl`), or
2. You explicitly invalidate:

```
# Invalidate a single path
dfx canister call cache_invalidation invalidate '("/")'

# Invalidate all dynamic assets
dfx canister call cache_invalidation invalidate_all
```

## Configuration

In `src/lib.rs`:

```rust
router_library::set_asset_config(router_library::AssetConfig {
    cache_config: router_library::CacheConfig {
        default_ttl: Some(Duration::from_secs(300)),
        per_route_ttl: HashMap::from([
            ("/ttl".to_string(), Duration::from_secs(30)),
        ]),
    },
    ..router_library::AssetConfig::default()
});
```

## Run

```
dfx start --background
dfx deploy
```

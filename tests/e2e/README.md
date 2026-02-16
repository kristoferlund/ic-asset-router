# E2E Tests (PocketIC)

End-to-end tests for the router library using [PocketIC](https://github.com/dfinity/pocketic) — a lightweight, deterministic IC replica for Rust test code.

## Prerequisites

1. **Rust toolchain** with the `wasm32-unknown-unknown` target:
   ```
   rustup target add wasm32-unknown-unknown
   ```

2. **PocketIC server binary** — the `pocket-ic` crate (v7+) automatically downloads and caches the server binary on first use. No manual setup is required.

   If you prefer to manage the binary manually, download it from
   [PocketIC releases](https://github.com/dfinity/pocketic/releases)
   and set:
   ```
   export POCKET_IC_BIN=/path/to/pocket-ic-server
   ```

## Running the tests

From the repository root:

```
cd tests/e2e && ./build_and_test.sh
```

This script:
1. Builds the test canister WASM (`tests/e2e/test_canister/`) in release mode
2. Runs the E2E test crate against the built WASM via PocketIC

Tests run single-threaded (`--test-threads=1`) to avoid port conflicts between PocketIC HTTP gateway instances.

## Test structure

```
tests/e2e/
  Cargo.toml            # Test crate: depends on pocket-ic, reqwest
  src/lib.rs            # setup() helper + all test functions
  build_and_test.sh     # Build WASM then run tests
  README.md             # This file
  test_canister/        # The canister deployed into PocketIC
    Cargo.toml
    build.rs
    dfx.json
    test_canister.did
    src/
      lib.rs            # Canister entry point
      routes/           # Route handlers exercised by tests
    static/
      style.css         # Static asset for serving tests
```

## Test scenarios

| Test | Validates |
|------|-----------|
| `test_static_asset_serving` | Static CSS served with correct MIME, 200, IC-Certificate header |
| `test_dynamic_route_first_request` | Query-update flow for dynamic route |
| `test_dynamic_route_cached_response` | Cached response matches, certification present |
| `test_parameter_extraction_posts` | `/posts/42` extracts param "42" |
| `test_parameter_extraction_echo` | `/echo/some-value` returns "some-value" |
| `test_wildcard_capture` | `/files/docs/2024/report.pdf` captures wildcard tail |
| `test_json_content_type` | `/json` returns application/json with correct body |
| `test_http_method_dispatch_get` | GET `/method_test` returns "get" |
| `test_http_method_dispatch_post` | POST `/method_test` returns "post" |
| `test_http_method_dispatch_405` | PUT `/method_test` returns 405 |
| `test_security_headers_present` | Security headers on dynamic response |
| `test_custom_404_handler` | Unknown path returns 404 with custom body |
| `test_middleware_header_injection` | Middleware adds X-Test-Middleware header |
| `test_cache_invalidation_via_update_call` | Re-request after cache population works |
| `test_ttl_expiry_regeneration` | Cached response persists without TTL |

## CI integration

In CI, ensure the `wasm32-unknown-unknown` target is installed, then run:

```yaml
- name: Run E2E tests
  run: cd tests/e2e && ./build_and_test.sh
```

The PocketIC server binary is downloaded automatically by the `pocket-ic` crate. If your CI environment restricts network access during test execution, pre-download the binary and set `POCKET_IC_BIN`.

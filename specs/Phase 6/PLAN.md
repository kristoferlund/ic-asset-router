# Phase 6 — Implementation Plan

**Scope:** Specs 6.1 through 6.9
**Library:** `~/gh/router_library/`
**Status:** Sessions 1–10 complete

## Dependency Order

```
6.3 (generated code warnings)  — independent, smallest change                    [DONE]
6.6 (minor fixes & cleanup)    — independent, small                              [DONE]
6.2 (wildcard in RouteContext)  — independent, modifies context.rs + build.rs     [DONE]
6.1 (not-found certification)  — independent, modifies lib.rs                    [DONE]
6.4 (example migration)        — depends on 6.2, 6.3                             [DONE]
6.5 (E2E test hardening)       — depends on 6.1, 6.2                             [DONE]
6.7 (single certified 404)     — depends on 6.1 (rewrites 6.1's implementation)  [DONE]
6.8 (expose URL utilities)     — independent                              [DONE]
6.9 (fix template paths)       — independent                              [DONE]
```

**Rationale (sessions 1–7):** Start with the two smallest, most isolated changes (6.3 warning suppression, 6.6 cleanup) — these touch build.rs and router.rs with no cross-cutting concerns. Then 6.2 (wildcard) and 6.1 (not-found cert) are independent of each other but both must land before 6.4 and 6.5. 6.4 (example migration) benefits from suppressed warnings and the wildcard field. 6.5 (E2E hardening) needs certified 404s and the wildcard context to update tests.

**Rationale (sessions 8–10):** 6.7 is the most critical remaining item — it reworks the per-path 404 certification from 6.1 into a single-entry fallback to eliminate the memory growth risk. 6.8 and 6.9 are independent small fixes that can run in any order after 6.7.

---

## Sessions

Each session heading below is a single OpenCode session. The agent completes all tasks in that session, commits, and stops. The next session picks up the next heading.

---

### Session 1: Spec 6.3 — Generated Code Warning Suppression

- [x] **6.3.1** In `src/build.rs`, at the top of the generated `__route_tree.rs` output (before the `use` statements at line ~141), emit `#[allow(unused_imports, unused_variables)]\n` as the first line of the generated file.
- [x] **6.3.2** In `src/build.rs`, in the `mod.rs` generation block (line ~616, where `children.concat()` is written), for each `pub mod _name;` line where the module name starts with `_`, prepend `#[allow(non_snake_case)]\n` to that specific `pub mod` declaration.
- [x] **6.3.3** In `src/router.rs:486`, change `fn test_request(path: &str) -> HttpRequest {` to `fn test_request(path: &str) -> HttpRequest<'_> {`.
- [x] **6.3.4** Verify: `cargo check` produces zero warnings in the library itself.
- [x] **6.3.5** Verify: `cargo test` passes.
- [x] **6.3.6** Verify: `cargo check --manifest-path examples/json-api/Cargo.toml` produces zero warnings from generated code (json-api uses `_itemId`, so this exercises the `non_snake_case` suppression).

**Commit:** `git add -A && git commit -m "spec 6.3: suppress warnings in generated code, fix test_request lifetime"`
**STOP after this session.**

---

### Session 2: Spec 6.6 — Minor Fixes & Cleanup

- [x] **6.6.1** In `src/build.rs`, create a new helper function `scan_pub_fns(path: &Path) -> Vec<String>` that reads the file and returns all `pub fn` names found (scanning line-by-line for `pub fn <name>(` pattern, same logic currently used by `has_pub_fn` and `detect_method_exports`).
- [x] **6.6.2** Refactor `has_pub_fn(path, name)` to call `scan_pub_fns(path)` and check if `name` is in the result.
- [x] **6.6.3** Refactor `detect_method_exports(path)` to call `scan_pub_fns(path)` and filter against `METHOD_NAMES`.
- [x] **6.6.4** Add a doc comment to `HandlerResultFn` in `src/router.rs:30` explaining the intentional divergence from the `RouteContext`-based public handler signature, noting that this is the internal (unwrapped) signature and that alignment would require wrapper generation in `__route_tree.rs` if `insert_result` is ever called from generated code.
- [x] **6.6.5** In `src/build.rs` test module, refactor `setup_temp_routes` to clean up after tests. Use `std::fs::remove_dir_all` in a cleanup guard or wrapper, or add a `cleanup_temp_routes(path)` helper called at the end of each test that uses `setup_temp_routes`.
- [x] **6.6.6** Verify: `cargo check` passes.
- [x] **6.6.7** Verify: `cargo test` passes — all existing tests produce the same results.

**Commit:** `git add -A && git commit -m "spec 6.6: unify scan helpers, document HandlerResultFn, clean up test temp dirs"`
**STOP after this session.**

---

### Session 3: Spec 6.2 — Wildcard Value in RouteContext

- [x] **6.2.1** Add `pub wildcard: Option<String>` field to `RouteContext<P, S>` in `src/context.rs` (after the `url` field, line ~75). Add a doc comment explaining it is `None` for routes without a wildcard segment and `Some(capture)` for catch-all routes.
- [x] **6.2.2** In `src/build.rs`, in the route handler wrapper generation (lines ~150-204), after constructing the `RouteContext` struct literal, add `wildcard: raw_params.get("*").cloned(),` for all wrappers. This is correct for both wildcard routes (where `"*"` exists in `raw_params`) and non-wildcard routes (where `get("*")` returns `None`).
- [x] **6.2.3** In `src/build.rs`, in the not-found handler wrapper (lines ~207-222), set `wildcard: None` in the `RouteContext` struct literal.
- [x] **6.2.4** Update `tests/e2e/test_canister/src/routes/files/all.rs` to use `ctx.wildcard.as_deref().unwrap_or("")` instead of the manual URL prefix-stripping workaround.
- [x] **6.2.5** Update doc examples for `RouteContext` in `src/context.rs` if any exist that show the struct literal — add the `wildcard` field.
- [x] **6.2.6** Verify: `cargo check` passes.
- [x] **6.2.7** Verify: `cargo test` passes.

**Commit:** `git add -A && git commit -m "spec 6.2: add wildcard field to RouteContext, simplify catch-all handlers"`
**STOP after this session.**

---

### Session 4: Spec 6.1 — Not-Found Response Certification

- [x] **6.1.1** In `src/lib.rs`, in the `http_request_update` function's `RouteResult::NotFound` branch (lines ~439-449), pipe the not-found handler's response through `certify_dynamic_response(response, &path)` before returning it. Apply this to both the custom handler path and the default plain-text 404 path.
- [x] **6.1.2** In `src/lib.rs`, in the `http_request` function's `RouteResult::NotFound` branch (lines ~285-308), after the static asset check fails, check `DYNAMIC_CACHE` for a previously certified response for this path. If found and not expired, attempt to serve it from `ASSET_ROUTER` via `serve_asset()`. If not found in cache, return `upgrade: true` to trigger the update path.
- [x] **6.1.3** Ensure that when `opts.certify` is `false` in `http_request`, the not-found handler still executes directly (no upgrade needed, same as current behavior for non-certified mode).
- [x] **6.1.4** Verify: `cargo check` passes.
- [x] **6.1.5** Verify: `cargo test` passes.

**Commit:** `git add -A && git commit -m "spec 6.1: certify not-found responses through standard dynamic pipeline"`
**STOP after this session.**

---

### Session 5: Spec 6.4 — Example Migration (htmx-app)

*Depends on: 6.2, 6.3 (sessions 1, 3 must be complete)*

- [x] **6.4.1** Rename `examples/htmx-app/src/routes/posts/:postId/` directory to `examples/htmx-app/src/routes/posts/_postId/`.
- [x] **6.4.2** Update all handler functions in `examples/htmx-app/src/routes/` to accept `RouteContext<Params>` (parameterized) or `RouteContext<()>` (static) instead of `(HttpRequest, RouteParams)`. Update param access to use `ctx.params.post_id` instead of `params.get("postId")`.
- [x] **6.4.3** Update `examples/htmx-app/src/lib.rs` `AssetConfig` struct literal to include `cache_config: router_library::CacheConfig::default()`.
- [x] **6.4.4** Delete the stale `examples/htmx-app/src/routes/posts/:postId/mod.rs` if the rename left it behind, and ensure `examples/htmx-app/src/routes/posts/_postId/mod.rs` is regenerated by the build script.
- [x] **6.4.5** Verify: `cargo check --manifest-path examples/htmx-app/Cargo.toml` succeeds with zero errors.
- [x] **6.4.6** Verify: `cargo build --target wasm32-unknown-unknown --manifest-path examples/htmx-app/Cargo.toml` succeeds.

**Commit:** `git add -A && git commit -m "spec 6.4: migrate htmx-app to _param convention and RouteContext"`
**STOP after this session.**

---

### Session 6: Spec 6.4 — Example Migration (askama-basic & tera-basic)

- [x] **6.4.7** Rewrite `examples/askama-basic/src/lib.rs` to use the file-based routing convention: create `examples/askama-basic/build.rs` calling `generate_routes()`, create `examples/askama-basic/src/routes/posts/_postId/index.rs` with the handler using `RouteContext<Params>`, add `mod route_tree { include!(concat!(env!("OUT_DIR"), "/__route_tree.rs")); }` to lib.rs, and wire up `ROUTES` thread_local via the generated route tree.
- [x] **6.4.8** Rewrite `examples/tera-basic/src/lib.rs` with the same pattern: add `build.rs`, create `src/routes/posts/_postId/index.rs`, use `RouteContext<Params>`, wire up via generated route tree and OUT_DIR include.
- [x] **6.4.9** Verify: `cargo check --manifest-path examples/askama-basic/Cargo.toml` succeeds.
- [x] **6.4.10** Verify: `cargo check --manifest-path examples/tera-basic/Cargo.toml` succeeds.

**Commit:** `git add -A && git commit -m "spec 6.4: migrate askama-basic and tera-basic to file-based routing and RouteContext"`
**STOP after this session.**

---

### Session 7: Spec 6.5 — E2E Test Hardening

*Depends on: 6.1, 6.2 (sessions 3, 4 must be complete)*

- [x] **6.5.1** Add `invalidate` and `invalidate_all` Candid update methods to `tests/e2e/test_canister/src/lib.rs`: `fn invalidate(path: String)` calls `router_library::invalidate_path(&path)`, `fn invalidate_all()` calls `router_library::invalidate_all_dynamic()`.
- [x] **6.5.2** Update `tests/e2e/test_canister/test_canister.did` to include the new method signatures: `invalidate : (text) -> ()` and `invalidate_all : () -> ()`.
- [x] **6.5.3** Add a `/ttl-test` route to the test canister (`tests/e2e/test_canister/src/routes/ttl_test.rs`) that returns the current IC time as a string. Configure `CacheConfig` in the test canister's `setup()` with a `per_route_ttl` entry for `/ttl-test` set to a short duration (e.g., 5 seconds).
- [x] **6.5.4** Replace `test_cache_invalidation_via_update_call` in `tests/e2e/src/lib.rs` with a proper invalidation test: request → cache → call `invalidate` via `pic.update_call()` → request again → verify fresh response.
- [x] **6.5.5** Add `test_ttl_expiry` in `tests/e2e/src/lib.rs`: request `/ttl-test` → cache → `pic.advance_time(Duration::from_secs(10))` + `pic.tick()` → request again → verify re-execution (different timestamp or successful response).
- [x] **6.5.6** Update `test_custom_404_handler` to assert status **404** (not 503), verify body contains `"custom 404"`, and verify `IC-Certificate` header is present on a second (cached) request.
- [x] **6.5.7** Verify: test canister compiles (`cargo build --target wasm32-unknown-unknown --manifest-path tests/e2e/test_canister/Cargo.toml`).
- [x] **6.5.8** Verify: all E2E tests pass via `tests/e2e/build_and_test.sh`.
- [x] **6.5.9** Verify: library unit tests still pass (`cargo test`).

**Commit:** `git add -A && git commit -m "spec 6.5: E2E invalidation endpoint, TTL test, certified 404 test"`
**STOP after this session.**

---

### Session 8: Spec 6.7 — Single Certified 404 Fallback

*Depends on: 6.1 (sessions 4, 7 must be complete — this session reworks the 6.1 implementation)*

- [x] **6.7.1** In `src/lib.rs`, define a constant `const NOT_FOUND_CANONICAL_PATH: &str = "/__not_found";` for the single canonical 404 cache key.
- [x] **6.7.2** Rewrite the `http_request_update` `RouteResult::NotFound` branch: execute the not-found handler once, then call `certify_dynamic_response(response, NOT_FOUND_CANONICAL_PATH)` — certifying at the canonical path, not the request path. This stores one entry in `DYNAMIC_CACHE` and one in `ASSET_ROUTER`.
- [x] **6.7.3** Rewrite the `http_request` `RouteResult::NotFound` branch (certified mode): after the static asset check fails, check `DYNAMIC_CACHE` for the canonical path `/__not_found`. If cached and not expired, serve from `ASSET_ROUTER` using a request rewritten to `/__not_found`. If not cached, return `upgrade: true`.
- [x] **6.7.4** Remove `serve_cached_non200()` function from `src/lib.rs`.
- [x] **6.7.5** Remove the non-200 branch from `certify_dynamic_response()` — restore it to 200-only certification through `AssetRouter`.
- [x] **6.7.6** Remove `CachedHttpResponse` struct from `src/assets.rs`. Remove `status_code` and `cached_response` fields from `CachedDynamicAsset`. Update all constructors and doc-tests.
- [x] **6.7.7** Verify: `cargo check` passes.
- [x] **6.7.8** Verify: `cargo test` passes.

**Commit:** `git add -A && git commit -m "spec 6.7: single certified 404 fallback, remove per-path non-200 certification"`
**STOP after this session.**

---

### Session 9: Spec 6.7 + 6.5 — 404 E2E Test Update & Verification

- [x] **6.7.9** Update `test_custom_404_handler` in `tests/e2e/src/lib.rs` to account for the new behavior: the 404 body is served but status may be 200 (since `serve_asset()` returns 200). Update assertions accordingly — verify the body contains "custom 404" and the response is certified. Document the status code behavior.
- [x] **6.7.10** Add a test verifying that requesting 100 different non-existent paths does NOT create 100 `DYNAMIC_CACHE` entries — only the canonical `/__not_found` entry should exist.
- [x] **6.7.11** Verify: test canister compiles for `wasm32-unknown-unknown`.
- [x] **6.7.12** Verify: all E2E tests pass via `build_and_test.sh`.
- [x] **6.7.13** Verify: `cargo test` passes.

**Commit:** `git add -A && git commit -m "spec 6.7: update E2E 404 test for single-entry fallback"`
**STOP after this session.**

---

### Session 10: Specs 6.8 + 6.9 — URL Utilities & Template Paths

- [x] **6.8.1** In `src/context.rs`, change `fn url_decode` to `pub fn url_decode`. Add a doc comment explaining it handles `%XX` decoding and `+`-as-space.
- [x] **6.8.2** Add `pub fn parse_form_body(body: &[u8]) -> HashMap<String, String>` to `src/context.rs` — parses `application/x-www-form-urlencoded` body using `url_decode`.
- [x] **6.8.3** Re-export `url_decode` and `parse_form_body` from `src/lib.rs`.
- [x] **6.8.4** Add unit tests for `url_decode`: percent-decoding, plus-as-space, malformed passthrough, plain passthrough.
- [x] **6.8.5** Add unit tests for `parse_form_body`: basic key-value pairs, plus decoding, empty body, encoded values.
- [x] **6.8.6** Replace the inline `parse_form_urlencoded` and `url_decode` in `examples/htmx-app/src/routes/posts/_postId/comments.rs` with `router_library::parse_form_body` and `router_library::url_decode`.
- [x] **6.9.1** In `examples/tera-basic/src/routes/posts/_postId/index.rs`, replace `include_str!("../../../../templates/post.html")` with `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/post.html"))`.
- [x] **6.9.2** Check all other examples for fragile relative `include_str!` paths and fix if found.
- [x] **6.9.3** Verify: `cargo test` passes.
- [x] **6.9.4** Verify: `cargo check --manifest-path examples/htmx-app/Cargo.toml` succeeds.

**Commit:** `git add -A && git commit -m "specs 6.8+6.9: expose URL utilities, fix fragile template paths"`
**STOP after this session.**

---

## Verification Protocol

After each session, the commit message and plan file updates serve as the completion signal.

After all 10 sessions are complete, run a final check:

```
cargo check
cargo test
cargo doc --no-deps
cargo check --manifest-path examples/htmx-app/Cargo.toml
cargo check --manifest-path examples/askama-basic/Cargo.toml
cargo check --manifest-path examples/tera-basic/Cargo.toml
cargo check --manifest-path examples/json-api/Cargo.toml
cd tests/e2e && ./build_and_test.sh
```

---

## Session Boundaries

The task listing above is organized into **10 explicit sessions**. Each session heading is a single OpenCode invocation. The agent completes the tasks in that session, commits, and stops. The next invocation starts a new session and picks up the next heading.

**When to stop early (before completing the session):**
- After a failed verification (`cargo check` or `cargo test` fails) and one fix attempt also fails — stop, start fresh. The next session sees the partial work in the code and the still-unchecked tasks.
- If the agent starts producing low-quality output — stop, context is degraded.

**After each session:**
- The agent must have committed its work before stopping
- Verify the plan file was updated (tasks checked off)

## Session Prompt Template

At the start of each OpenCode session, use this prompt:

```
You are implementing changes to a Rust library at ~/gh/router_library/.

PLAN FILE: ~/bee/BEE NOTES/Projects/Asset Router/Phase 6/PLAN.md
SPEC FILES: ~/bee/BEE NOTES/Projects/Asset Router/Phase 6/

Instructions:
1. Read the plan file (PLAN.md).
2. Find the next incomplete session (the first session heading with unchecked tasks).
3. Read the corresponding spec file for context (the spec is named in the session heading).
4. Study the library source files relevant to that session's tasks.
5. Implement each unchecked task in that session, in order.
6. After each task: run `cargo check`. If it fails, fix before continuing.
7. After completing all tasks in the session: run `cargo test`. Fix any failures.
8. Mark completed tasks in PLAN.md (change `[ ]` to `[x]`).
9. Append a session summary to SESSION.md (in the same directory as PLAN.md).
   The summary must include:
   - A heading with the session name (e.g., "## Session 3: Spec 6.2 — Wildcard Value in RouteContext")
   - Date
   - What was accomplished (tasks completed, brief description)
   - Obstacles encountered (compilation errors, test failures, unclear specs, workarounds applied)
   - Out-of-scope observations: anything noticed during implementation that should be
     addressed elsewhere in the codebase but was not part of this session's tasks.
     These may become new specs or tasks in future phases.
   Create the file if it does not exist. Append to it if it does.
10. Commit all changes using the commit message specified at the end of the session.
11. STOP. Do not continue to the next session.

Rules:
- Do not skip tasks. Do not reorder tasks within a session.
- If `cargo check` or `cargo test` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Do not modify files outside ~/gh/router_library/ except for PLAN.md and SESSION.md.
- ALWAYS commit before stopping. Every session must end with a git commit.
```

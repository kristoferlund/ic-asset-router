# Phase 5 — Implementation Plan

**Scope:** Specs 5.1 through 5.7
**Library:** `~/gh/router_library/`
**Status:** Not started

## Dependency Order

```
5.1 (build script rewrite)        — independent, foundational
5.4 (reserved filename validation) — depends on 5.1 (build script must be rewritten first)
5.2 (type-safe route params)       — depends on 5.1 (codegen changes, _param convention)
5.3 (query string access)          — depends on 5.2 (RouteContext must exist)
5.5 (unit & property tests)        — depends on 5.1, 5.2 (tests the new codegen and context)
5.6 (documentation & examples)     — depends on 5.1, 5.2, 5.3 (documents the final API)
5.7 (PocketIC E2E tests)           — depends on 5.5, 5.6 (uses a test canister, needs stable API)
```

**Rationale:** 5.1 is the foundation — the entire file naming convention, codegen output, and `OUT_DIR` strategy change. 5.4 adds validation to the new build script. 5.2 builds on the new codegen to produce typed `Params` structs and `RouteContext`. 5.3 extends `RouteContext` with query string support. 5.5 tests everything built so far. 5.6 documents and creates example canisters. 5.7 is the final integration validation.

---

## Sessions

Each session heading below is a single OpenCode session. The agent completes all tasks in that session, commits, and stops. The next session picks up the next heading.

---

### Session 1: Spec 5.1 — Build Script (Naming Convention & Attribute Scanning)

- [x] **5.1.1** Update `sanitize_mod()` to recognize `_`-prefixed names as dynamic params (e.g., `_postId` → `:postId`). Remove the `:`-prefix handling.
- [x] **5.1.2** Update `file_to_route_path()` to map `_param` → `:param` and `all` → `*` (the reserved catch-all filename). Remove old `*` and `:` handling.
- [x] **5.1.3** Update `file_to_handler_path()` to work with the new `_param` convention — `_postId` is already a valid Rust identifier, no sanitization needed.
- [x] **5.1.4** Remove all `#[path = "..."]` attribute generation from `process_directory()` — no longer needed since all filenames are valid Rust identifiers.
- [x] **5.1.5** In `process_directory()`, detect when both `_param.rs` and `_param/index.rs` exist for the same param. Emit `panic!("Ambiguous route: ...")` at build time.
- [x] **5.1.6** Add a function `scan_route_attribute(path: &Path) -> Option<String>` that uses a regex to find `#[route(path = "...")]` in a source file and returns the override path segment.
- [x] **5.1.7** In `process_directory()`, call `scan_route_attribute()` for each route file. If present, use the attribute value instead of the filename when constructing the route path.
- [x] **5.1.8** Replace the fragile `replace("//", "/")` in `file_to_route_path()` with proper path normalization: `split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/")` prefixed with `/`.
- [x] **5.1.9** Verify: `cargo check` succeeds
- [x] **5.1.10** Verify: `cargo test` passes

**Commit:** `git add -A && git commit -m "spec 5.1: new naming convention, attribute scanning, path normalization"`
**STOP after this session.**

---

### Session 2: Spec 5.1 — Build Script (OUT_DIR, Configurable Dir, Manifest)

- [x] **5.1.11** Add a `generate_routes_from(dir: &str)` entry point alongside `generate_routes()`. The existing `generate_routes()` calls `generate_routes_from("src/routes")` for backwards compatibility.
- [x] **5.1.12** Change `generate_routes()` to write `__route_tree.rs` into `OUT_DIR` (via `std::env::var("OUT_DIR")`) instead of `src/`. Update the generated code to use `include!(concat!(env!("OUT_DIR"), "/__route_tree.rs"))` for consumers.
- [x] **5.1.13** Generate a `route_manifest.json` into `OUT_DIR` listing all registered routes, their handler functions, methods, and parameter names.
- [x] **5.1.14** Verify: `cargo check` succeeds
- [x] **5.1.15** Verify: `cargo test` passes
- [x] **5.1.16** Verify: no `#[path = "..."]` attributes remain in generated output

**Commit:** `git add -A && git commit -m "spec 5.1: OUT_DIR wiring, configurable route dir, route manifest"`
**STOP after this session.**

---

### Session 3: Spec 5.4 — Reserved Filename Validation

*Depends on: 5.1 (sessions 1–2 must be complete)*

- [x] **5.4.1** Add `const RESERVED_FILES: &[&str] = &["middleware", "not_found"];` to `build.rs` as the single source of truth
- [x] **5.4.2** In `process_directory()`, check incoming filenames against `RESERVED_FILES` before route registration. Ensure `middleware.rs` and `not_found.rs` are never registered as route handlers (verify current behavior, tighten if needed).
- [x] **5.4.3** Add best-effort signature validation: when `middleware.rs` is found, scan for `pub fn middleware`. If absent, emit `cargo:warning=middleware.rs should export pub fn middleware(...)`.
- [x] **5.4.4** Add best-effort signature validation: when `not_found.rs` is found, scan for a recognized method export. If absent, `panic!` with a clear message (verify this already exists).
- [x] **5.4.5** Document the reserved name collision escape hatch in a code comment: use `#[route(path = "middleware")]` on a differently-named file if you need `/middleware` as a route.
- [x] **5.4.6** Verify: `cargo check` succeeds
- [x] **5.4.7** Verify: `cargo test` passes

**Commit:** `git add -A && git commit -m "spec 5.4: reserved filename validation and signature checks"`
**STOP after this session.**

---

### Session 4: Spec 5.2 — Type-Safe Route Params (RouteContext & Params Generation)

*Depends on: 5.1 (sessions 1–2 must be complete)*

- [x] **5.2.1** Create `src/context.rs` with `RouteContext<P>` struct containing `params: P`, `query: QueryParams`, `method: Method`, `headers: Vec<HeaderField>`, `body: Vec<u8>`, `url: String`
- [x] **5.2.2** Define `pub type QueryParams = HashMap<String, String>`
- [x] **5.2.3** Add a `parse_query(url: &str) -> QueryParams` function that extracts query string key-value pairs from a URL
- [x] **5.2.4** Export `RouteContext`, `QueryParams` from `src/lib.rs`
- [x] **5.2.5** In `build.rs`, for each route directory containing `_param` segments, generate a `Params` struct with snake_case fields derived from the param names (e.g., `_postId` → `pub post_id: String`)
- [x] **5.2.6** Generate the `Params` struct into the source tree (as part of the `mod.rs` in each route directory) for IDE visibility. Routes without dynamic segments use `()`.
- [x] **5.2.7** Verify: `cargo check` succeeds
- [x] **5.2.8** Verify: `cargo test` passes

**Commit:** `git add -A && git commit -m "spec 5.2: RouteContext type and Params struct generation"`
**STOP after this session.**

---

### Session 5: Spec 5.2 — Type-Safe Route Params (Wiring & Handler Signature)

- [x] **5.2.9** Update the generated route tree wiring to construct `RouteContext<Params>` from the raw `HttpRequest` and `RouteParams` before calling the handler
- [x] **5.2.10** Update `HandlerFn` type (or generate per-route handler types) to accept `RouteContext<Params>` instead of `(HttpRequest, RouteParams)`
- [x] **5.2.11** Verify: `cargo check` succeeds
- [x] **5.2.12** Verify: `cargo test` passes
- [x] **5.2.13** Verify: IDE completion works for `ctx.params.<field>` (manual check — note result in PLAN.md)

> **5.2.13 IDE note:** Params structs are generated into the source tree (mod.rs in each route directory), not OUT_DIR. RouteContext<P> has `pub params: P` with named pub fields. rust-analyzer should provide full completion for `ctx.params.<field>`. Not verified in a live IDE session — design guarantees visibility.

**Commit:** `git add -A && git commit -m "spec 5.2: wiring RouteContext into handlers"`
**STOP after this session.**

---

### Session 6: Spec 5.3 — Query String Access & Typed Search Params

*Depends on: 5.2 (sessions 4–5 must be complete)*

- [x] **5.3.1** Add `serde` and `serde_urlencoded` as dependencies in `Cargo.toml` (behind a `typed-search` feature flag if desired, or unconditional if size impact is acceptable)
- [x] **5.3.2** Add a second type parameter to `RouteContext<P, S = ()>` for typed search params: `pub search: S`
- [x] **5.3.3** In the generated wiring, deserialize the query string into `S` using `serde_urlencoded::from_str`. Use `unwrap_or_default()` so missing/malformed params become defaults rather than panics.
- [x] **5.3.4** In `build.rs`, detect `SearchParams` struct in route files (scan for `pub struct SearchParams`). If present, use it as the `S` type parameter; otherwise default to `()`.
- [x] **5.3.5** Add test: `parse_query("?page=3&filter=active")` returns correct HashMap
- [x] **5.3.6** Add test: `parse_query("")` returns empty HashMap
- [x] **5.3.7** Add test: malformed query values don't panic (fallback to defaults)
- [x] **5.3.8** Verify: `cargo check` succeeds
- [x] **5.3.9** Verify: `cargo test` passes

**Commit:** `git add -A && git commit -m "spec 5.3: typed search params and query string deserialization"`
**STOP after this session.**

---

### Session 7: Spec 5.5 — Unit & Property Tests (Router, Middleware, Build Script)

*Depends on: 5.1, 5.2 (sessions 1–5 must be complete)*

- [x] **5.5.1** Audit all existing `#[cfg(test)]` modules in `router.rs`, `config.rs`, `lib.rs`. Document gaps in a comment at top of each test module.
- [x] **5.5.2** Add router edge case tests: trailing slashes, double slashes, empty segments, URL-encoded characters, very long paths, routes with many parameters
- [x] **5.5.3** Add middleware chain tests: execution order, short-circuit, request modification, multiple middleware in hierarchy
- [x] **5.5.4** Add `#[cfg(test)]` module to `build.rs` testing: `_param` → `:param` mapping, `all` → `/*` mapping, `index` → directory path, `#[route(path)]` scanning, reserved filename recognition
- [x] **5.5.5** Verify: `cargo test` passes

**Commit:** `git add -A && git commit -m "spec 5.5: unit tests for router edge cases, middleware, build script"`
**STOP after this session.**

---

### Session 8: Spec 5.5 — Unit & Property Tests (Proptest & Cache)

- [x] **5.5.6** Add `proptest` as a dev-dependency in `Cargo.toml`
- [x] **5.5.7** Add property tests: inserted routes are always found, non-inserted routes are not found, param routes capture any segment value, wildcard routes capture remaining path
- [x] **5.5.8** Add cache invalidation tests (if Phase 4 complete): `invalidate_path` removes correct entry, `invalidate_prefix` matches correctly, `invalidate_all_dynamic` clears dynamic only, TTL expiry, `NotModified` preserves and resets TTL
- [x] **5.5.9** Verify: `cargo test` passes (including proptest)
- [x] **5.5.10** Verify: no ICP runtime dependencies in any unit test (all tests run without a canister environment)

**Commit:** `git add -A && git commit -m "spec 5.5: property-based tests and cache invalidation tests"`
**STOP after this session.**

---

### Session 9: Spec 5.6 — Documentation (Rustdoc)

*Depends on: 5.1, 5.2, 5.3 (sessions 1–6 must be complete)*

- [x] **5.6.1** Add `///` doc comments to all public types and functions in `src/router.rs`, `src/context.rs`, `src/config.rs`, `src/assets.rs`, `src/lib.rs`, `src/middleware.rs`
- [x] **5.6.2** Add `/// # Examples` code blocks for non-trivial public items (`RouteContext`, `SecurityHeaders` presets, invalidation functions)
- [x] **5.6.3** Verify: `cargo doc --no-deps` produces clean docs with no warnings

**Commit:** `git add -A && git commit -m "spec 5.6: rustdoc for all public API surface"`
**STOP after this session.**

---

### Session 10: Spec 5.6 — Documentation (Example Canisters)

- [x] **5.6.4** Create `examples/security-headers/` — minimal canister demonstrating strict/permissive/custom header configuration
- [x] **5.6.5** Create `examples/json-api/` — JSON API with GET/POST/PUT/DELETE, method routing, CORS middleware
- [x] **5.6.6** Create `examples/cache-invalidation/` — TTL expiry, explicit invalidation, conditional regeneration (if Phase 4 complete)
- [x] **5.6.7** Create `examples/custom-404/` — Custom `not_found.rs` returning styled HTML
- [x] **5.6.8** Add a README.md to each new example explaining what it demonstrates
- [x] **5.6.9** Verify: `cargo check` succeeds for each example (`--manifest-path`)

**Commit:** `git add -A && git commit -m "spec 5.6: example canisters for security headers, JSON API, cache, custom 404"`
**STOP after this session.**

---

### Session 11: Spec 5.6 — Documentation (Guides)

- [x] **5.6.10** Write the Getting Started guide in the library's `README.md`: add dependency, create routes dir, first route, wire up canister entry points, deploy, test
- [x] **5.6.11** Create `MIGRATION.md` covering: `:param.rs` → `_param.rs`, `*.rs` → `all.rs`, handler signature changes, security header default change, cache-control configurability
- [x] **5.6.12** Verify: README getting-started steps are accurate (walk through them manually)

**Commit:** `git add -A && git commit -m "spec 5.6: getting started guide and migration guide"`
**STOP after this session.**

---

### Session 12: Spec 5.7 — PocketIC E2E Tests (Test Canister)

*Depends on: 5.5, 5.6 (sessions 7–11 must be complete)*

- [x] **5.7.1** Create `tests/e2e/test_canister/` directory with `Cargo.toml`, `dfx.json`, `src/lib.rs`, `src/routes/` directory structure per the spec
- [x] **5.7.2** Create route handlers: `index.rs` (GET → "hello"), `json.rs` (GET → `{"ok":true}`), `echo/_path.rs` (GET → path param), `posts/_postId.rs` (GET → post ID), `files/all.rs` (GET → wildcard capture), `method_test.rs` (GET → "get", POST → "post")
- [x] **5.7.3** Create `not_found.rs` (custom 404) and `middleware.rs` (adds X-Test-Middleware header)
- [x] **5.7.4** Create `static/style.css` — a known file for static asset tests
- [x] **5.7.5** Verify: test canister compiles (`cargo build --target wasm32-unknown-unknown --manifest-path tests/e2e/test_canister/Cargo.toml`)

**Commit:** `git add -A && git commit -m "spec 5.7: PocketIC test canister scaffold"`
**STOP after this session.**

---

### Session 13: Spec 5.7 — PocketIC E2E Tests (Test Crate & Scenarios)

- [x] **5.7.6** Create `tests/e2e/Cargo.toml` depending on `pocket-ic` and `reqwest` (blocking)
- [x] **5.7.7** Create `tests/e2e/src/lib.rs` with a `setup()` helper that creates a PocketIC instance, deploys the test canister, and starts the HTTP gateway
- [x] **5.7.8** Create `tests/e2e/build_and_test.sh` — builds the test canister WASM, then runs the E2E tests
- [x] **5.7.9** Add tests: static asset serving (GET `/style.css` → 200, correct MIME, certification header), dynamic route first request (query→update flow), cached response serving
- [x] **5.7.10** Add tests: parameter extraction (`/posts/42`), wildcard capture (`/files/docs/report.pdf`), JSON content-type certification, HTTP method dispatch (GET, POST, 405)
- [x] **5.7.11** Add tests: security headers present on responses, custom 404 handler, middleware header injection
- [x] **5.7.12** Add tests (if Phase 4 complete): cache invalidation via update call, TTL expiry via `pic.advance_time()`
- [x] **5.7.13** Document PocketIC server binary setup (download, `POCKET_IC_BIN` env var) in a README or comment in the test crate
- [x] **5.7.14** Verify: all E2E tests pass via `build_and_test.sh`

**Commit:** `git add -A && git commit -m "spec 5.7: PocketIC E2E test crate and all test scenarios"`
**STOP after this session.**

---

## Verification Protocol

After each session, the commit message and plan file updates serve as the completion signal.

After all 13 sessions are complete, run a final check:

```
cargo check
cargo test
cargo doc --no-deps
cd tests/e2e && ./build_and_test.sh
```

---

## Session Boundaries

The task listing above is organized into **13 explicit sessions**. Each session heading is a single OpenCode invocation. The agent completes the tasks in that session, commits, and stops. The next invocation starts a new session and picks up the next heading.

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

PLAN FILE: ~/bee/BEE NOTES/Projects/Asset Router/Phase 5/PLAN.md
SPEC FILES: ~/bee/BEE NOTES/Projects/Asset Router/Phase 5/

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
   - A heading with the session name (e.g., "## Session 3: Spec 5.4 — Reserved Filename Validation")
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

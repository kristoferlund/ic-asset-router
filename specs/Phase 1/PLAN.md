# Phase 1 — Implementation Plan

**Scope:** Specs 1.1 through 1.7
**Library:** `~/gh/router_library/`
**Status:** Complete

## Dependency Order

```
1.4 (debug logging)     — independent, no deps
1.3 (error handling)    — independent, no deps
1.2 (response metadata) — independent, no deps
1.5 (wildcard capture)  — independent, no deps
1.6 (minor fixes)       — independent, no deps
1.1 (configurable headers) — independent, no deps
1.7 (cache-control)     — depends on 1.1 (adds to AssetConfig)
```

Recommended execution order: start with the smallest/most isolated changes (1.4, 1.6, 1.3, 1.2, 1.5) to build confidence in the codebase, then tackle the larger 1.1, then 1.7 which builds on 1.1.

---

## Tasks

### 1.4 — Remove Debug Logging

- [x] **1.4.1** Add `[features]` section to `Cargo.toml` with `debug-logging` feature flag
- [x] **1.4.2** Create `debug_log!` macro (in `src/lib.rs` or `src/macros.rs`) with conditional compilation: expands to `ic_cdk::println!` when `debug-logging` is enabled, expands to nothing otherwise
- [x] **1.4.3** Replace all `ic_cdk::println!` calls in `src/lib.rs` with `debug_log!`
- [x] **1.4.4** Replace all `ic_cdk::println!` calls in `src/router.rs` with `debug_log!`
- [x] **1.4.5** Verify: `cargo check` succeeds with no features (default)
- [x] **1.4.6** Verify: `cargo check --features debug-logging` succeeds
- [x] **1.4.7** Verify: no `ic_cdk::println!` calls remain outside the macro definition

### 1.6 — Minor Fixes (non-deferred items only)

- [x] **1.6.1** Fix typo in `src/build.rs:6`: "Gnereates" → "Generates"
- [x] **1.6.2** Add missing MIME types to `src/mime.rs`: `avif` → `image/avif`, `heic` → `image/heic`, `csv` → `text/csv`, `yaml`/`yml` → `application/yaml`, `map` → `application/json`, `webmanifest` → `application/manifest+json`
- [x] **1.6.3** Evaluate `collect_assets` in `src/assets.rs` — if unused externally, make `pub(crate)` or remove; if used by consumers, add a doc comment explaining its purpose
- [x] **1.6.4** Fix skip-certification path in `src/lib.rs`: replace hardcoded `"/"` in `HttpCertificationPath::exact(...)` with the actual request path
- [x] **1.6.5** Audit `Cargo.toml` dependencies: remove `ic-canister-sig-creation` if unused; check if `candid` and `serde` are needed directly or only transitively
- [x] **1.6.6** Verify: `cargo check` succeeds after all changes
- [x] **1.6.7** Verify: existing tests in `src/router.rs` still pass (`cargo test`)

### 1.3 — Graceful Error Handling

- [x] **1.3.1** Create helper function `error_response(status: u16, message: &str) -> HttpResponse<'static>` in `src/lib.rs`
- [x] **1.3.2** Replace `req.get_path().unwrap()` at `src/lib.rs:40` (in `http_request`) with match that returns 400 on error
- [x] **1.3.3** Replace `req.get_path().unwrap()` at `src/lib.rs:91` (in `http_request_update`) with match that returns 400 on error
- [x] **1.3.4** Audit all remaining `unwrap()` calls in `src/lib.rs` — replace any that operate on request/external data with proper error responses
- [x] **1.3.5** Add tests: malformed URL returns 400 (not a trap)
- [x] **1.3.6** Add tests: missing content-type in handler response doesn't trap
- [x] **1.3.7** Verify: `cargo test` passes

### 1.2 — Handler-Controlled Response Metadata

- [x] **1.2.1** In `http_request_update` (`src/lib.rs`), extract `content-type` from the handler's returned `HttpResponse` headers instead of hardcoding `"text/html"`
- [x] **1.2.2** Add fallback: if handler response has no `content-type` header, use `"application/octet-stream"`
- [x] **1.2.3** Use the extracted content-type in the `AssetConfig::File` passed to `asset_router.certify_assets()`
- [x] **1.2.4** Add test: handler returning `content-type: application/json` produces correct certification metadata
- [x] **1.2.5** Add test: handler returning `content-type: text/html` still works (regression)
- [x] **1.2.6** Add test: handler with no content-type header falls back to `application/octet-stream`
- [x] **1.2.7** Verify: `cargo test` passes

### 1.5 — Wildcard Segment Capture

- [x] **1.5.1** Modify `_match()` in `src/router.rs`: when wildcard node matches, join remaining segments with `/` and insert as `params["*"]`
- [x] **1.5.2** Ensure the wildcard key `"*"` is populated alongside any named params from parent segments
- [x] **1.5.3** Add test: `/*` captures full remaining path (`/a/b/c` → `params["*"] = "a/b/c"`)
- [x] **1.5.4** Add test: `/files/*` captures tail (`/files/docs/report.pdf` → `params["*"] = "docs/report.pdf"`)
- [x] **1.5.5** Add test: mixed params and wildcard (`/users/:id/files/*` → `params["id"] = "42"`, `params["*"] = "docs/report.pdf"`)
- [x] **1.5.6** Add test: empty wildcard match (`/files/` matching `/files/*` → `params["*"] = ""`)
- [x] **1.5.7** Verify: all existing tests still pass (wildcard tests will need updated expectations since they previously expected empty params)
- [x] **1.5.8** Verify: `cargo test` passes

### 1.1 — Configurable Headers

- [x] **1.1.1** Create `src/config.rs` with `SecurityHeaders` struct (11 fields, all `Option<String>`)
- [x] **1.1.2** Implement `SecurityHeaders::strict()` — reproduces current hardcoded values plus CORP, X-DNS-Prefetch-Control, X-Permitted-Cross-Domain-Policies
- [x] **1.1.3** Implement `SecurityHeaders::permissive()` — allows cross-origin resources, SAMEORIGIN framing
- [x] **1.1.4** Implement `SecurityHeaders::none()` — all fields `None`
- [x] **1.1.5** Implement `Default for SecurityHeaders` — returns `permissive()`
- [x] **1.1.6** Create `AssetConfig` struct with `security_headers: SecurityHeaders` and `custom_headers: Vec<HeaderField>`
- [x] **1.1.7** Implement `Default for AssetConfig`
- [x] **1.1.8** Add `SecurityHeaders::to_headers(&self) -> Vec<HeaderField>` method that converts non-None fields to header tuples
- [x] **1.1.9** Implement header merging logic: security headers → custom_headers → per-route headers (last-write-wins for duplicate names)
- [x] **1.1.10** Add `AssetConfig` as a parameter to the library's initialization path — store in thread-local state alongside `ASSET_ROUTER`
- [x] **1.1.11** Modify `get_asset_headers()` in `src/assets.rs` to use the configured `SecurityHeaders` instead of hardcoded values
- [x] **1.1.12** Modify `certify_all_assets()` to use the configured `AssetConfig`
- [x] **1.1.13** Modify `http_request_update()` to use the configured `AssetConfig` when certifying dynamic responses
- [x] **1.1.14** Export `SecurityHeaders`, `AssetConfig` from `src/lib.rs`
- [x] **1.1.15** Add test: `strict()` produces the expected set of headers
- [x] **1.1.16** Add test: `permissive()` produces the expected set of headers
- [x] **1.1.17** Add test: `none()` produces zero headers
- [x] **1.1.18** Add test: custom headers override security headers of the same name
- [x] **1.1.19** Add test: `X-XSS-Protection` is never set by any preset
- [x] **1.1.20** Verify: `cargo test` passes
- [x] **1.1.21** Verify: `cargo check` passes

### 1.7 — Configurable Cache-Control

*Depends on: 1.1 (AssetConfig and config.rs must exist)*

- [x] **1.7.1** Add `CacheControl` struct to `src/config.rs` with `static_assets` and `dynamic_assets` fields
- [x] **1.7.2** Implement `Default for CacheControl` — reproduces current hardcoded values
- [x] **1.7.3** Add `cache_control: CacheControl` field to `AssetConfig`
- [x] **1.7.4** Modify `collect_assets_with_config()` in `src/assets.rs` to use `CacheControl::static_assets` instead of the hardcoded `IMMUTABLE_ASSET_CACHE_CONTROL`
- [x] **1.7.5** Modify `http_request_update()` in `src/lib.rs` to use `CacheControl::dynamic_assets` instead of the hardcoded `NO_CACHE_ASSET_CACHE_CONTROL`
- [x] **1.7.6** Remove the hardcoded constants `IMMUTABLE_ASSET_CACHE_CONTROL` and `NO_CACHE_ASSET_CACHE_CONTROL` from `src/assets.rs` (or keep as documentation, but they should not be the source of truth)
- [x] **1.7.7** Add test: default `CacheControl` reproduces current behavior
- [x] **1.7.8** Add test: custom static cache-control value is used for static assets
- [x] **1.7.9** Add test: custom dynamic cache-control value is used for dynamic responses
- [x] **1.7.10** Verify: `cargo test` passes

---

## Verification Protocol

After each spec is complete, run:

```
cargo check
cargo test
```

Both must pass before marking the spec as done.

After all Phase 1 specs are complete, run a final check:

```
cargo check
cargo test
cargo doc --no-deps  (should produce no warnings for public items)
```

---

## Session Boundaries

Each OpenCode session works on **one spec** (e.g., all tasks under 1.4). When all tasks for that spec are checked off and verified, the session ends. The next session picks up the next spec.

**Why one spec per session:**
- Context stays focused on one concern (logging, error handling, headers, etc.)
- No context pollution from unrelated file reads and failed attempts
- The plan file carries state between sessions -- nothing is lost
- If the agent degrades mid-spec, you lose at most one spec's worth of work

**When to stop early (before completing the spec):**
- After a failed verification (`cargo check` or `cargo test` fails) and one fix attempt also fails -- stop, start fresh. The next session sees the partial work in the code and the still-unchecked task in the plan.
- If the agent starts producing low-quality output (repeating itself, making contradictory changes, ignoring test failures) -- stop. Context is degraded.

**After each session:**
- The agent must have committed its work before stopping
- Verify the plan file was updated (tasks checked off)

## Session Prompt Template

At the start of each OpenCode session, use this prompt:

```
You are implementing changes to a Rust library at ~/gh/router_library/.

PLAN FILE: ~/bee/BEE NOTES/Projects/Asset Router/Phase 1/PLAN.md
SPEC FILES: ~/bee/BEE NOTES/Projects/Asset Router/Phase 1/

Instructions:
1. Read the plan file (PLAN.md).
2. Find the next incomplete spec (the first spec group with unchecked tasks).
3. Read the corresponding spec file for that spec (e.g., "1.4 — Remove Debug Logging.md").
4. Study the library source files relevant to that spec.
5. Implement each unchecked task in that spec group, in order.
6. After each task: run `cargo check`. If it fails, fix before continuing.
7. After completing all tasks in the spec group: run `cargo test`. Fix any failures.
8. Mark completed tasks in PLAN.md (change `[ ]` to `[x]`).
9. Append a session summary to SESSION.md (in the same directory as PLAN.md).
   The summary must include:
   - A heading with the session name (e.g., "## Session: Spec 1.4 — Remove Debug Logging")
   - Date
   - What was accomplished (tasks completed, brief description)
   - Obstacles encountered (compilation errors, test failures, unclear specs, workarounds applied)
   - Out-of-scope observations: anything noticed during implementation that should be
     addressed elsewhere in the codebase but was not part of this session's tasks.
     These may become new specs or tasks in future phases.
   Create the file if it does not exist. Append to it if it does.
10. Commit all changes: `git add -A && git commit -m "spec X.X: <brief description>"`
11. STOP after completing one spec group. Do not continue to the next spec.

Rules:
- Do not skip tasks. Do not reorder tasks within a spec group.
- If `cargo check` or `cargo test` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Ask before installing new dependencies.
- Do not modify files outside ~/gh/router_library/ except for PLAN.md and SESSION.md.
- ALWAYS commit before stopping. Every session must end with a git commit.
```

Adjust the prompt to request a specific spec if you want to override the sequential order.

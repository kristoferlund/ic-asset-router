# Phase 7 — Configurable Certification Modes: Implementation Plan

**Scope:** Specs 7.1 through 7.6
**Target codebase:** `/Users/kristoferlund/gh/ic-asset-router`
**Status:** Not started

---

## Dependency Order

```
7.1  Define Certification Configuration Types
 │
 ▼
7.2  Build Asset Router  (depends on 7.1)
 │
 ▼
7.3  Refactor certify_assets  (depends on 7.2)
 │
 ▼
7.4  Per-Route Certification Configuration  (depends on 7.1, 7.2)
 │
 ├──▶ 7.5  Documentation and Examples  (depends on 7.1–7.4)
 │
 └──▶ 7.6  Integration Tests  (depends on 7.1–7.3)
```

**Execution order:** 7.1 → 7.2 → 7.3 → 7.4 → 7.6 → 7.5

**Rationale:**
- 7.1 is pure types with no dependencies — smallest, most isolated change.
- 7.2 builds the new AssetRouter on top of 7.1 types and is the largest spec.
- 7.3 is a thin refactor of `certify_assets` that wires 7.1 types into the 7.2 router.
- 7.4 introduces the proc-macro crate and build-script integration, needing the types (7.1) and router (7.2) to exist.
- 7.6 (tests) comes before 7.5 (docs) because tests validate the implementation and may surface issues; docs should describe the final verified behavior.
- 7.5 is last — documentation and examples only make sense once everything works.

**Note on illustrative code in specs:** The specs contain pseudocode that communicates design intent but does not always match exact upstream API signatures (e.g., `ic-http-certification` CEL builder methods accept `&[&str]` not `Vec<String>`, and `HttpCertification::response_only()` takes a typed CEL expression struct, not a `String`). During implementation, consult the actual `ic-http-certification` 3.0.3 API docs for correct types and signatures. The existing codebase (`src/lib.rs` lines 274–293) has working examples of the certification API.

---

## Tasks

### 7.1 — Define Certification Configuration Types

- [x] **7.1.1** Create `src/certification.rs` with the `CertificationMode` enum (`Skip`, `ResponseOnly(ResponseOnlyConfig)`, `Full(FullConfig)`) and its `Default` impl (returns `ResponseOnly`).
- [x] **7.1.2** Implement `ResponseOnlyConfig` with `include_headers` and `exclude_headers` fields and its `Default` impl (wildcard include, exclude `date` / `ic-certificate` / `ic-certificate-expression`).
- [x] **7.1.3** Implement `FullConfig` with `request_headers`, `query_params`, and `response: ResponseOnlyConfig` fields and its `Default` impl.
- [x] **7.1.4** Implement `FullConfigBuilder` with `with_request_headers`, `with_query_params`, `with_response_headers`, `excluding_response_headers`, and `build`. Ensure header name normalization (lowercase) in every `with_*_headers` method.
- [x] **7.1.5** Implement convenience constructors: `CertificationMode::skip()`, `CertificationMode::response_only()`, and `CertificationMode::authenticated()`.
- [x] **7.1.6** Add `pub mod certification;` to `src/lib.rs` and export the new types.
- [x] **7.1.7** Write unit tests: default is `ResponseOnly`, `skip()` produces `Skip`, `response_only()` has correct default config, `authenticated()` has `authorization` in request_headers and `content-type` in response, builder with all/partial/no options, header normalization.
- [x] **7.1.8** Verify: `cargo check` and `cargo test` pass.

---

### 7.2 — Build Asset Router

Depends on: 7.1

This is the largest spec. Split into three sub-sections: types, core implementation, and migration.

#### 7.2A — Types and structure

- [x] **7.2.1** Create `src/asset_router.rs` (module root). Define `AssetEncoding` enum, `CertifiedAsset` struct, `AssetCertificationConfig` struct (including `certified_at` and `ttl` fields), and `AssetRouterError` enum.
- [x] **7.2.2** Implement `CertifiedAsset::is_dynamic()` and `CertifiedAsset::is_expired()`.
- [x] **7.2.3** Define the `AssetRouter` struct with fields: `assets: HashMap<String, CertifiedAsset>`, `aliases: HashMap<String, String>`, `fallbacks: Vec<(String, String)>`, `tree: Rc<RefCell<HttpCertificationTree>>`. Implement `with_tree()`, `root_hash()`, `contains_asset()`, `get_asset()`, `get_asset_mut()`.
- [x] **7.2.4** Verify: `cargo check` passes with the new module compiled.

#### 7.2B — Core implementation

- [x] **7.2.5** Implement `build_cel_expression()` — a function that takes `&CertificationMode` and produces the typed CEL expression needed by `ic-http-certification`. Handle all three modes (`Skip`, `ResponseOnly`, `Full`) including `with_request_headers` and `with_request_query_parameters` for Full mode. Use the existing codebase's certification patterns (see `src/lib.rs:274–293`) as reference for correct API usage.
- [x] **7.2.6** Implement `AssetRouter::certify_asset()` for `Skip` and `ResponseOnly` modes. Return `FullModeRequiresRequest` error for `Full` mode. Handle encoding map construction, tree insertion, fallback registration (sorted by scope length), and alias registration.
- [x] **7.2.7** Implement `AssetRouter::certify_dynamic_asset()` for all three modes including `Full` (takes `&HttpRequest` + `&HttpResponse`).
- [x] **7.2.8** Implement `AssetRouter::serve_asset()` — exact match → alias resolution → sorted fallback lookup. Handle encoding negotiation (Brotli > Gzip > Identity). Return `(HttpResponse, witness, expr_path)` tuple; Skip mode returns empty witness.
- [x] **7.2.9** Implement `AssetRouter::delete_asset()` — resolve alias, remove from tree, remove aliases and fallback entries.
- [x] **7.2.10** Verify: `cargo check` passes.

#### 7.2C — Tests

- [x] **7.2.11** Write unit tests for `certify_asset` (ResponseOnly success, Skip success, Full returns error), `certify_dynamic_asset` (Full success, ResponseOnly/Skip also work).
- [x] **7.2.12** Write unit tests for `serve_asset` (exact match, alias resolution, fallback match, longest-prefix fallback wins, no match returns None, Skip has no witness, encoding negotiation).
- [x] **7.2.13** Write unit tests for `delete_asset` (removes asset + aliases + fallback, delete via alias, delete nonexistent is no-op), `root_hash` changes after certify/delete, re-certification replaces old, mode switching.
- [x] **7.2.14** Write unit tests for `is_dynamic`, `is_expired` on `CertifiedAsset`.
- [x] **7.2.15** Verify: `cargo test` passes (all new and existing tests).

---

### 7.3 — Refactor certify_assets for Certification Modes

Depends on: 7.2

- [x] **7.3.1** Rename `certify_all_assets` to `certify_assets` in `src/assets.rs`. Implement it as a one-liner that delegates to `certify_assets_with_mode` with `CertificationMode::response_only()`.
- [x] **7.3.2** Implement `certify_assets_with_mode` — walks the directory recursively (files + subdirs), builds `AssetCertificationConfig` for each file, calls `router.certify_asset()`, then calls `certified_data_set`.
- [x] **7.3.3** Migrate `src/lib.rs` thread-local state: replace `ic_asset_certification::AssetRouter` with the new `crate::asset_router::AssetRouter`. Remove the `DYNAMIC_CACHE` thread-local. Update all call sites (`certify_dynamic_response`, `invalidate_path`, `invalidate_prefix`, `invalidate_all_dynamic`, `http_request`, `http_request_update`) to use the unified router.
- [x] **7.3.4** Update `src/lib.rs` exports: export `certify_assets`, `certify_assets_with_mode`, and deprecate or remove `certify_all_assets`.
- [x] **7.3.5** Update `Cargo.toml`: remove `ic-asset-certification` from `[dependencies]` (or make it optional).
- [x] **7.3.6** Verify: `cargo check` passes, `cargo test` passes (all existing tests still work).

---

### 7.4 — Per-Route Certification Configuration

Depends on: 7.1, 7.2

#### 7.4A — Types and proc-macro crate

- [ ] **7.4.1** Create `src/route_config.rs` with the `RouteConfig` struct (`certification: CertificationMode`, `ttl: Option<Duration>`, `headers: Vec<HeaderField>`) and its `Default` impl.
- [ ] **7.4.2** Export `RouteConfig` from `src/lib.rs`.
- [ ] **7.4.3** Create `macros/` proc-macro subcrate: `macros/Cargo.toml` (with `proc-macro2`, `quote`, `syn 2.0`), `macros/src/lib.rs` with the `#[route]` attribute macro. Parse `certification = "skip"` / `"response_only"` / `"authenticated"` presets and `certification = custom(...)` syntax. Generate a `__route_config()` function (not a static) returning `RouteConfig`.
- [ ] **7.4.4** Add `macros` as a path dependency in the root `Cargo.toml`. Re-export `#[route]` from `src/lib.rs`.
- [ ] **7.4.5** Verify: `cargo check` passes for both the macros crate and the main crate.

#### 7.4B — Build script and runtime integration

- [ ] **7.4.6** Update `src/build.rs` to detect `#[route(...)]` attributes on handler functions and reference the generated `__route_config()` function in the route tree output.
- [ ] **7.4.7** Update `src/router.rs` to store `RouteConfig` per route in `RouteNode`. Modify `RouteEntry` (or equivalent) to carry the config.
- [ ] **7.4.8** Update `certify_dynamic_response` (or its replacement) to use the route's `RouteConfig.certification` mode when certifying — dispatching to `certify_asset` for Skip/ResponseOnly or `certify_dynamic_asset` for Full mode.
- [ ] **7.4.9** Write unit tests: macro parses all preset strings, custom syntax produces correct `FullConfig`, routes without attribute default to `ResponseOnly`.
- [ ] **7.4.10** Verify: `cargo check` and `cargo test` pass.

---

### 7.6 — Integration Tests for Certification Modes

Depends on: 7.1, 7.2, 7.3

- [ ] **7.6.1** Add test dependencies to `Cargo.toml` or `tests/e2e/Cargo.toml`: `pocket-ic`, `ic-certificate-verification` (or equivalent).
- [ ] **7.6.2** Create test canister fixture in `tests/` (or extend existing `tests/e2e/test_canister/`) that exposes all certification modes: Skip, ResponseOnly, and authenticated (Full) routes.
- [ ] **7.6.3** Create shared test helpers (`setup_canister`, `query`, `query_with_header`, `update`, `verify_response`).
- [ ] **7.6.4** Write PocketIC integration tests: Skip mode (no `ic-certificate` header), ResponseOnly (certificate present and verifies), Full/authenticated (valid with correct auth, invalid with different auth), query params (valid with matching, invalid with different).
- [ ] **7.6.5** Write PocketIC integration tests: mixed modes in single canister, dynamic route with Skip, dynamic route with authenticated Full cycle, invalidation with unified router.
- [ ] **7.6.6** Add certification tests to CI (`.github/workflows/test.yml` or equivalent).
- [ ] **7.6.7** Verify: all integration tests pass via `cargo test` or the e2e test script.

---

### 7.5 — Documentation and Examples for Certification Modes

Depends on: 7.1, 7.2, 7.3, 7.4

- [ ] **7.5.1** Add rustdoc to all new public types (`CertificationMode`, `ResponseOnlyConfig`, `FullConfig`, `FullConfigBuilder`, `RouteConfig`, `AssetRouter`, `CertifiedAsset`, `AssetCertificationConfig`). Include "when to use" guidance and code examples.
- [ ] **7.5.2** Add a "Certification Modes" section to the crate-level doc comment in `src/lib.rs` (or create `docs/certification.md`). Include the decision tree, header selection guide, and performance comparison.
- [ ] **7.5.3** Create `examples/certification-modes/` example canister demonstrating Skip, ResponseOnly, authenticated, and custom modes.
- [ ] **7.5.4** Create `examples/api-authentication/` example canister demonstrating why authenticated endpoints need Full certification.
- [ ] **7.5.5** Update `README.md` with a certification section referencing the new modes and linking to examples.
- [ ] **7.5.6** Verify: `cargo doc --no-deps` builds without warnings. Examples compile with `cargo check`.

---

## Verification Protocol

### After each spec group

```sh
cargo check                     # Compilation
cargo test                      # All unit tests
cargo doc --no-deps 2>&1 | grep -i warning   # Doc warnings (should be empty)
```

### After all specs complete

```sh
cargo check
cargo test
cargo doc --no-deps
# E2E tests (if PocketIC is available):
cd tests/e2e && bash build_and_test.sh
```

---

## Session Boundaries

- Each session works on **one spec group only** (e.g., "7.1" or "7.2A+7.2B+7.2C"), then stops.
- Spec 7.2 is large — it may be split across sessions by sub-section (7.2A, 7.2B, 7.2C), but all sub-sections must complete before moving to 7.3.
- If a verification step fails **twice in a row** after attempted fixes, mark the failing task with `[!]`, git commit all partial work, and stop.
- The agent **must git commit** all changes before stopping, every time.
- Do **not** continue to the next spec group after completing one — stop and let the user start a new session.

---

## Session Prompt Template

Paste this into a new agent session to execute the next spec group:

~~~
Read the implementation plan at:
  /Users/kristoferlund/gh/ic-asset-router/specs/Phase 7/PLAN.md

Find the first spec group that has incomplete tasks (unchecked `- [ ]` items).
Read the corresponding spec file in:
  /Users/kristoferlund/gh/ic-asset-router/specs/Phase 7/

Study the relevant source files in the target codebase at:
  /Users/kristoferlund/gh/ic-asset-router/

Then implement the tasks for that ONE spec group, in order. Follow these rules:

1. Implement tasks sequentially — no skipping, no reordering.
2. After each task, run the verification command (`cargo check` or `cargo test` as appropriate).
3. Mark each task complete in PLAN.md (`- [x]`) as you finish it.
4. If verification fails, fix the issue and retry. If it fails twice on the same task, mark it with `- [!]` in PLAN.md, git commit partial work, and STOP.
5. Only modify files in the target codebase (`/Users/kristoferlund/gh/ic-asset-router/`), PLAN.md, and SESSION.md. Do not modify the spec files.
6. When all tasks in the spec group are done, run the full verification protocol:
   ```
   cargo check && cargo test && cargo doc --no-deps
   ```
7. Git commit all changes with a descriptive message.
8. Append a session summary to:
     /Users/kristoferlund/gh/ic-asset-router/specs/Phase 7/SESSION.md
   Include:
   - Heading: `## Session N: Spec X.Y — <title>`
   - Date
   - Tasks completed and brief description
   - Obstacles encountered (compilation errors, test failures, unclear specs, workarounds)
   - Out-of-scope observations (things noticed that should be addressed elsewhere)
9. STOP. Do not continue to the next spec group.

IMPORTANT: The spec code snippets are illustrative pseudocode. The actual
`ic-http-certification` 3.0.3 API uses typed CEL expression structs (not
strings) and `&[&str]` slices (not `Vec<String>`). Consult the existing
codebase patterns in `src/lib.rs` (lines 274–293) and the upstream docs at
https://docs.rs/ic-http-certification/3.0.3/ for correct API usage.
~~~

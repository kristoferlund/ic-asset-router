# Phase 7 — Session Log

## Session 1: Spec 7.1 — Define Certification Configuration Types

**Date:** 2026-02-18

### Tasks completed

- **7.1.1** Created `src/certification.rs` with the `CertificationMode` enum (`Skip`, `ResponseOnly(ResponseOnlyConfig)`, `Full(FullConfig)`) and `Default` impl (returns `ResponseOnly`).
- **7.1.2** Implemented `ResponseOnlyConfig` with `include_headers` (wildcard) and `exclude_headers` (`date`, `ic-certificate`, `ic-certificate-expression`) and `Default` impl.
- **7.1.3** Implemented `FullConfig` with `request_headers`, `query_params`, `response: ResponseOnlyConfig` fields and `Default` impl (empty request headers/query params, default response config).
- **7.1.4** Implemented `FullConfigBuilder` with `with_request_headers`, `with_query_params`, `with_response_headers`, `excluding_response_headers`, and `build`. All `with_*_headers` methods normalize to lowercase.
- **7.1.5** Implemented convenience constructors: `CertificationMode::skip()`, `CertificationMode::response_only()`, and `CertificationMode::authenticated()` (full cert with `authorization` request header and `content-type` response header).
- **7.1.6** Added `pub mod certification;` to `src/lib.rs` and re-exported `CertificationMode`, `FullConfig`, `FullConfigBuilder`, `ResponseOnlyConfig`.
- **7.1.7** Wrote 11 unit tests covering: default mode, skip constructor, response-only defaults, authenticated preset, builder with all/partial/no options, header normalization, FullConfig/ResponseOnlyConfig defaults, Clone+Debug traits.
- **7.1.8** Verified: `cargo check`, `cargo test` (193 tests pass), `cargo doc --no-deps` (no warnings).

### Obstacles encountered

None. The spec was clear and the types are standalone with no external dependencies beyond what already existed in the crate.

### Out-of-scope observations

- The spec pseudocode shows `Vec<String>` for header fields, but the upstream `ic-http-certification` API uses `&[&str]` slices and `Cow<'a, [&'a str]>` in `DefaultResponseCertification`. The translation from our `Vec<String>` config types to the upstream API will need to happen in spec 7.2 (`build_cel_expression`), where owned strings are converted to borrowed slices.

---

## Session 2: Spec 7.2 — Build Asset Router

**Date:** 2026-02-18

### Tasks completed

All 15 tasks (7.2.1–7.2.15) implemented successfully.

#### 7.2A — Types and structure (7.2.1–7.2.4)

- **7.2.1** Created `src/asset_router.rs` with `AssetEncoding` enum (Identity, Gzip, Brotli, Deflate), `CertifiedAsset` struct (multi-encoding map, aliases, fallback prefix, certification tree entries, `certified_at`, `ttl`), `AssetCertificationConfig` struct, and `AssetRouterError` enum.
- **7.2.2** Implemented `CertifiedAsset::is_dynamic()` (checks `ttl.is_some()`) and `CertifiedAsset::is_expired()` (compares `certified_at + ttl` against `ic_cdk::api::time()`; static assets never expire).
- **7.2.3** Defined `AssetRouter` struct with `assets`, `aliases`, `fallbacks`, and shared `Rc<RefCell<HttpCertificationTree>>`. Implemented `with_tree()`, `root_hash()`, `contains_asset()`, `get_asset()`, `get_asset_mut()`.
- **7.2.4** Verified `cargo check` passes.

#### 7.2B — Core implementation (7.2.5–7.2.10)

- **7.2.5** Implemented `build_cel_expression_string()` — returns the CEL expression string for all three modes. For Skip: `DefaultCelBuilder::skip_certification()`. For ResponseOnly: builds `DefaultResponseCertification` from config's include/exclude headers, then `DefaultCelBuilder::response_only_certification().with_response_certification()`. For Full: includes request headers, query params, and response certification via `DefaultCelBuilder::full_certification()`.
- **7.2.6** Implemented `certify_asset()` for Skip and ResponseOnly modes. Returns `FullModeRequiresRequest` error for Full mode. Handles encoding map construction, tree insertion via `HttpCertificationTreeEntry`, fallback registration (sorted by prefix length descending), and alias registration (index.html → directory paths).
- **7.2.7** Implemented `certify_dynamic_asset()` for all three modes including Full. Builds a certification copy of the response with the `IC-CertificateExpression` header injected, constructs the appropriate `HttpCertification` variant, and inserts into the tree.
- **7.2.8** Implemented `serve_asset()` with exact match → alias resolution → sorted fallback lookup. Encoding negotiation prefers Brotli > Gzip > Identity. Returns `(HttpResponse, HashTree, Vec<String>)` tuple; Skip mode returns `ic_certification::empty()` witness and empty expr_path.
- **7.2.9** Implemented `delete_asset()` — resolves alias to canonical path, removes tree entries, cleans up aliases and fallback entries.
- **7.2.10** Verified `cargo check` passes.

#### 7.2C — Tests (7.2.11–7.2.15)

- **7.2.11** 11 tests for `certify_asset` (ResponseOnly success, Skip success, Full returns error, auto content-type detection, explicit content-type, encoding storage, alias registration, fallback registration, additional headers, certified_at/ttl storage) and `certify_dynamic_asset` (Full, ResponseOnly, Skip).
- **7.2.12** 10 tests for `serve_asset` (exact match, alias resolution, fallback match, longest-prefix fallback wins, no match returns None, Skip has empty witness/expr_path, encoding negotiation for brotli/gzip/identity, CEL expression header present, additional headers in response).
- **7.2.13** 8 tests for `delete_asset` (removes asset + tree entry, removes aliases, removes fallback, delete via alias resolves canonical, nonexistent is no-op), `root_hash` changes (after certify, after delete), re-certification replaces old hash, mode switching (certify-delete-recertify).
- **7.2.14** 4 tests for `is_dynamic` (true when ttl present, false when absent) and `is_expired` (respects certified_at + ttl, static assets never expire).
- **7.2.15** Verified all 240 tests pass (193 existing + 47 new). `cargo doc --no-deps` clean.

### Obstacles encountered

- **Lifetime invariance in `DefaultResponseCertification<'a>`**: Could not store `'static` CEL expression structs that borrow from local `Vec<String>`. Solved by constructing CEL expressions inline in `create_certification()` and `create_certification_with_request()` helpers rather than as stored intermediaries.
- **`HttpCertification::response_only()` requires `IC-CertificateExpression` header**: In `certify_dynamic_asset()`, the caller's response may not have this header. Solved by building a certification-only copy of the response with the header added, used solely for `HttpCertification` construction.
- **`ic_certification::empty()` for Skip mode witness**: `HashTree::Pruned` is not a public constructor. Used `ic_certification::empty()` which creates a valid empty `HashTree` for the Skip mode witness.
- **`DefaultRequestCertification` import path**: Not re-exported at root of `ic-http-certification` — must import from `ic_http_certification::cel::DefaultRequestCertification`.

### Out-of-scope observations

- The `DYNAMIC_CACHE` thread-local in `src/lib.rs` and the `ic_asset_certification::AssetRouter` usage will be replaced in spec 7.3 when `certify_assets` is refactored to use the new `AssetRouter`.
- The `ic-http-certification` crate resolves to 3.1.0 despite Cargo.toml specifying 3.0.3 — API is compatible but the version should be updated in Cargo.toml during 7.3.

---

## Session 3: Spec 7.3 — Refactor certify_assets for Certification Modes

**Date:** 2026-02-18

### Tasks completed

All 6 tasks (7.3.1–7.3.6) implemented successfully across sessions 3a and 3b.

#### Session 3a (7.3.1–7.3.4)

- **7.3.1** Renamed `certify_all_assets` to `certify_assets` in `src/assets.rs`. Kept `certify_all_assets` as a deprecated backward-compatible alias delegating to `certify_assets`.
- **7.3.2** Implemented `certify_assets_with_mode` with a new `certify_dir_recursive` function that walks `include_dir` directories, builds `AssetCertificationConfig` per file, calls `router.certify_asset()`, auto-generates aliases for `index.html` files (directory root + trailing slash), and collects `.br`/`.gz` sibling files as pre-compressed encoding variants. Files ending in `.br` or `.gz` are skipped as primary assets.
- **7.3.3** Full migration of `src/lib.rs`:
  - Replaced `ic_asset_certification::AssetRouter` thread-local with `crate::asset_router::AssetRouter`.
  - Removed `DYNAMIC_CACHE` thread-local entirely.
  - Updated all 6 `serve_asset` call sites to use the new API (`Option<(HttpResponse, HashTree, Vec<String>)>`) plus `add_v2_certificate_header`.
  - Rewrote `certify_dynamic_response_inner` to delete existing assets before re-certifying, using `Option<String>` for fallback scope.
  - Updated all TTL/cache-state logic in `http_request`/`http_request_update` to query `ASSET_ROUTER` instead of `DYNAMIC_CACHE`.
  - Rewrote invalidation functions (`invalidate_path`, `invalidate_prefix`, `invalidate_all_dynamic`) and all tests.
  - Removed `CachedDynamicAsset` type.
  - Added `dynamic_paths()` and `dynamic_paths_with_prefix()` methods to `AssetRouter` to replace `DYNAMIC_CACHE` lookups.
- **7.3.4** Updated `src/lib.rs` exports to include `certify_assets` and `certify_assets_with_mode`.

#### Session 3b (7.3.5–7.3.6)

- **7.3.5** Removed `ic-asset-certification` from `Cargo.toml` dependencies. Confirmed no source files import from `ic_asset_certification` (only doc comments reference it).
- **7.3.6** Full verification: `cargo check` clean, all 240 tests pass, `cargo doc --no-deps` builds without warnings.

### Obstacles encountered

- **`ic_asset_certification` handled encoding internally** via filename-based lookups (`.br`, `.gz` sibling files). The new `certify_dir_recursive` replicates this by detecting sibling files in the `include_dir` directory tree. Files ending in `.br`/`.gz` are skipped as primary assets and instead collected as encoding variants of the base file.
- **`DYNAMIC_CACHE` removal required new `AssetRouter` methods**: The invalidation functions previously iterated over `DYNAMIC_CACHE` to find dynamic asset paths. Added `dynamic_paths()` and `dynamic_paths_with_prefix()` to `AssetRouter` to expose this information.
- **`certified_data_set()` unavailable in unit tests**: This IC runtime API cannot be called outside a canister. Tests manipulate `ASSET_ROUTER` directly via `with_borrow_mut` to test invalidation logic without hitting IC APIs.
- **`collect_assets` public function removed**: It depended on `ic_asset_certification::Asset` type which no longer exists. The function was not used externally (no grep hits outside the crate).

### Out-of-scope observations

- Examples (`react-app`, `htmx-app`) and `tests/e2e/test_canister` still call `certify_all_assets` — they work via the deprecated alias but should be updated in a future session (spec 7.5 — Documentation and Examples).
- The `ic-http-certification` Cargo.toml version specifies `3.0.3` but resolves to `3.1.0` — consider updating the version string for clarity.

---

## Session 4: Spec 7.4 — Per-Route Certification Configuration

**Date:** 2026-02-18

### Tasks completed

All 10 tasks (7.4.1–7.4.10) implemented successfully.

#### 7.4A — Types and proc-macro crate (7.4.1–7.4.5)

- **7.4.1** Created `src/route_config.rs` with `RouteConfig` struct containing `certification: CertificationMode`, `ttl: Option<Duration>`, and `headers: Vec<HeaderField>` fields, plus `Default` impl (defaults to `ResponseOnly`, no TTL, no extra headers).
- **7.4.2** Exported `RouteConfig` from `src/lib.rs` via `pub mod route_config` and `pub use`.
- **7.4.3** Created `macros/` proc-macro subcrate with `#[route]` attribute macro. Parses preset strings (`"skip"`, `"response_only"`, `"authenticated"`), custom syntax (`certification = custom(request_headers = [...], query_params = [...], response_headers = [...])`) with TTL support (`ttl = <seconds>`), and `path = "..."` for build script integration. Generates `__route_config()` function returning `RouteConfig`.
- **7.4.4** Added `ic-asset-router-macros` as path dependency in root `Cargo.toml`. Re-exported `#[route]` macro from `src/lib.rs`.
- **7.4.5** Both crates compile cleanly.

#### 7.4B — Build script and runtime integration (7.4.6–7.4.10)

- **7.4.6** Updated `src/build.rs`: added `scan_certification_attribute()` function that parses `#[route(certification = ...)]` attributes from source files, added `has_certification_attribute` and `module_path` fields to `MethodExport`, generated `set_route_config()` calls with appropriate `RouteConfig` construction in `__route_tree.rs` output. Added 8 unit tests for attribute scanning and directory processing.
- **7.4.7** Updated `src/router.rs`: added `route_configs: HashMap<String, RouteConfig>` to root `RouteNode`, implemented `set_route_config()` and `get_route_config()` methods. Extended `RouteResult::Found` from 3-tuple to 4-tuple adding route pattern string. Modified `_match()` to reconstruct the matched route pattern as it traverses the trie, returning it in the result. Updated all existing test destructurings to 4-field pattern. Added 10 new tests for config storage, retrieval, and pattern matching.
- **7.4.8** Updated `src/lib.rs`: `http_request_update` looks up `RouteConfig` via matched route pattern, passes certification mode to `certify_dynamic_response_inner`. Extended `certify_dynamic_response_inner` with `mode` and `request` parameters — dispatches to `certify_dynamic_asset()` for `Full` mode and `certify_asset()` for `Skip`/`ResponseOnly`. Added `certify_dynamic_response_with_ttl` for per-route TTL overrides. Updated `http_request` and `http_request_update` to handle the new 4-field `RouteResult::Found`.
- **7.4.9** Wrote 21 new tests across modules: 6 tests for `scan_certification_attribute` (presets, custom syntax, no attribute, TTL), 2 tests for `process_directory` cert detection, 3 tests for `RouteConfig` defaults, 4 tests for router `set_route_config`/`get_route_config`, 6 tests for `resolve` pattern matching with config lookup.
- **7.4.10** Full verification passed: `cargo check` clean, 261 unit tests + 7 doc tests pass, `cargo doc --no-deps` builds without warnings.

### Obstacles encountered

- **`NodeType` enum accidentally removed**: During a large edit on `router.rs`, the `NodeType` enum was deleted. Had to re-add it with the correct variants (`Static`, `Param`, `CatchAll`).
- **`RouteResult::Found` breaking change**: Extending from 3 to 4 fields required updating every destructuring site in `router.rs` tests (13+ locations) and all match arms in `lib.rs` (6+ locations).
- **Route pattern reconstruction in `_match()`**: The trie traversal needed to reconstruct the matched pattern string (e.g., `"/:id/edit"`) as it recurses. Solved by threading a `current_pattern` parameter through the recursive `_match` method and appending each segment's contribution.
- **Doc link warning**: `[CacheConfig]` in `route_config.rs` doc comments produced a rustdoc warning. Fixed by using the fully qualified `[crate::config::CacheConfig]` path.

### Out-of-scope observations

- The `#[route]` macro generates `__route_config()` functions but the build script detection is heuristic (regex-based source scanning). A more robust approach would use `syn` to parse the source files, but this would add build-time dependencies.
- Integration testing of the full macro → build script → router pipeline (end-to-end with actual `#[route]` attributes on handler functions compiled by the build script) is deferred to spec 7.6.
- The `PROMPT.md` file in the repo root appears to be a leftover and is untracked — should be cleaned up.

---

## Session 5: Spec 7.6 — Integration Tests (E2E Verification)

**Date:** 2026-02-18

### Tasks completed

Task 7.6.8 — the final task in spec group 7.6. All prior tasks (7.6.1–7.6.7) were completed in Session 4. This session fixed 6 failing e2e tests and verified all pass.

#### Fixes applied to pass e2e tests (26/26)

1. **Skip-mode certification (`asset_router.rs`)**: Skip-mode responses now generate a real witness from the certification tree instead of returning `ic_certification::empty()`. The IC boundary node requires an `ic-certificate` header with a valid skip proof even for skip-certified responses.

2. **Fallback/404 certification paths (`asset_router.rs`)**: Fallback assets (e.g., `/__not_found` 404 handler) now use `HttpCertificationPath::wildcard(scope)` instead of `HttpCertificationPath::exact(path)`. Exact paths only validate for that specific URL, but fallbacks serve any URL under the scope.

3. **Response status code preservation (`asset_router.rs`, `lib.rs`)**: Added a `status_code` field to `CertifiedAsset` and `AssetCertificationConfig` so 404 fallback responses preserve their original status code instead of being hardcoded to 200 OK in `serve_matched_asset()`.

4. **Dynamic asset tracking (`asset_router.rs`, `lib.rs`, `assets.rs`)**: Added an explicit `dynamic: bool` field to `CertifiedAsset` and `AssetCertificationConfig`. Previously `is_dynamic()` relied on `ttl.is_some()`, which missed dynamically-generated assets without TTL (like 404 responses). The flag is set to `true` by `certify_dynamic_response_with_ttl()`.

5. **E2e test expectations (`tests/e2e/src/lib.rs`)**: Updated 3 Skip-mode test assertions to expect an `ic-certificate` header (with skip proof) instead of asserting no header.

#### Verification

- `cargo check` — pass
- `cargo test` — 262 unit tests pass, 7 doc tests pass
- `cargo doc --no-deps` — builds without warnings

### Obstacles encountered

- **Skip-mode witness generation**: The initial assumption was that Skip responses don't need an `ic-certificate` header, but the IC boundary node requires it for all certified responses. Had to trace through how the certification tree generates witnesses for skip CEL expressions.
- **Fallback wildcard vs exact paths**: The distinction between `HttpCertificationPath::exact()` and `HttpCertificationPath::wildcard()` was subtle — fallbacks must use wildcard to match arbitrary URLs under their scope, otherwise certificate verification fails for any URL that isn't the exact fallback path.
- **Dynamic asset detection**: The `ttl.is_some()` heuristic for `is_dynamic()` broke for dynamically-generated 404 responses that don't have a TTL. Introducing an explicit `dynamic` flag resolved this cleanly and also fixed the auth route invalidation tests.

### Out-of-scope observations

- All spec 7.6 tasks are now complete. The remaining incomplete spec group is 7.5 (Documentation and Examples).

---

## Session 6: Spec 7.5 — Documentation and Examples for Certification Modes

**Date:** 2026-02-18

### Tasks completed

All 6 tasks (7.5.1–7.5.6) implemented successfully.

- **7.5.1** Added comprehensive rustdoc to all new public types:
  - `CertificationMode` — "When to Use Each Variant" table, code examples for `default()`, `skip()`, `authenticated()`.
  - `ResponseOnlyConfig` — header selection strategies (wildcard with exclusions vs explicit inclusion), default explanation.
  - `FullConfig` — "When to Use" guidance, "Which Headers to Certify" advice, code example.
  - `FullConfigBuilder` — enhanced example showing all builder methods.
  - `RouteConfig` — defaults table, `#[route]` macro usage examples.
  - `AssetRouter` — lookup order documentation (exact → alias → fallback), thread safety note.
  - `CertifiedAsset` — description of stored state and why `Clone` is not derived.
  - `AssetCertificationConfig` — default behavior description.
  - `AssetEncoding` — encoding preference order (Brotli > Gzip > Identity).
  - Module-level doc for `asset_router` — capabilities list.

- **7.5.2** Added "Certification Modes" section to `src/lib.rs` crate-level doc comment with:
  - Decision table (mode → when to use → example routes).
  - Code examples for Response-only (default), Skip, Authenticated, Custom Full.
  - Programmatic configuration section for `certify_assets_with_mode`.
  - Performance comparison table (relative cost and witness size).
  - Common mistakes section (over-certifying, under-certifying, non-deterministic data).

- **7.5.3** Created `examples/certification-modes/` example canister with 4 routes:
  - `GET /` — response-only (default, no attribute)
  - `GET /public/health` — skip certification
  - `GET /api/user` — authenticated (full certification)
  - `GET /content/articles` — custom full with query params `page` and `limit`

- **7.5.4** Created `examples/api-authentication/` focused example canister with 2 routes:
  - `GET /` — public about page (response-only, explains the concept)
  - `GET /profile` — authenticated endpoint showing why Authorization header certification prevents cross-user response mixing

- **7.5.5** Updated `README.md`:
  - Added "Certification Modes" section with choosing-a-mode table, code examples for all 4 modes, and programmatic configuration for static assets.
  - Updated the features bullet point to mention configurable certification modes.
  - Added `certification-modes` and `api-authentication` to the examples table.

- **7.5.6** Full verification passed:
  - `cargo check` — clean
  - `cargo test` — 262 unit tests pass, 10 doc tests pass (3 new doc tests added)
  - `cargo doc --no-deps` — builds without warnings
  - Both new examples compile with `cargo check`

### Obstacles encountered

- **`QueryParams` type mismatch**: Initial `articles.rs` handler used `.first()` on query values, but `QueryParams` is `HashMap<String, String>` (not `HashMap<String, Vec<String>>`). Fixed to use `.get()` directly.
- No other obstacles — this was a documentation-only spec with no logic changes.

### Out-of-scope observations

- All Phase 7 spec groups (7.1–7.6, 7.5) are now complete. The implementation plan has no remaining unchecked tasks.
- The two new examples follow the established canister pattern but don't include static asset directories — they're pure route-handler examples. If the project later adds a convention for example READMEs, these could be enhanced.

---

## Session 7: Spec 7.7 — RouteContext Ergonomic Improvements

**Date:** 2026-02-18

### Tasks completed

All 11 tasks (7.7.1–7.7.11) implemented successfully.

#### 7.7.1–7.7.4: New convenience methods on `RouteContext`

- **7.7.1** Added `RouteContext::header(&self, name: &str) -> Option<&str>` — case-insensitive lookup using `eq_ignore_ascii_case`, first match wins, zero-copy borrow from the existing `Vec<HeaderField>`.
- **7.7.2** Added `RouteContext::body_to_str(&self) -> Result<&str, Utf8Error>` — strict UTF-8 check via `std::str::from_utf8`, zero-copy.
- **7.7.3** Added `RouteContext::json<T: DeserializeOwned>(&self) -> Result<T, JsonBodyError>` with `JsonBodyError` enum (Utf8/Json variants, implements `Display` + `Error`). Added `serde_json = "1.0"` to `[dependencies]`.
- **7.7.4** Added `RouteContext::form_data(&self) -> HashMap<String, String>` (wraps `parse_form_body`, infallible) and `RouteContext::form<T: DeserializeOwned>(&self) -> Result<T, FormBodyError>` with `FormBodyError` enum (Utf8/Deserialize variants, implements `Display` + `Error`).

#### 7.7.5: Exports

- **7.7.5** Exported `JsonBodyError` and `FormBodyError` from `src/lib.rs` via the `context` module re-export.

#### 7.7.6: Unit tests

- **7.7.6** Added 17 new unit tests in `src/context.rs` with a `test_ctx` helper: `header` (case-insensitive, missing, first-match-wins), `body_to_str` (valid, invalid UTF-8, empty), `json` (valid, invalid JSON, invalid UTF-8, empty body), `form_data` (basic, empty, url-encoded), `form` (valid, missing field, invalid UTF-8, empty body with optional fields).

#### 7.7.7: Example and e2e test migration

- **7.7.7** Migrated all verbose patterns across examples and tests:
  - 3 `eq_ignore_ascii_case` header lookups → `ctx.header()` (in `api-authentication`, `certification-modes`, `test_canister/auth_test.rs`)
  - 2 `String::from_utf8_lossy` + `serde_json::from_str` patterns → `ctx.json()` (in `json-api` items index and _itemId)
  - 1 `parse_form_body(&ctx.body)` → `ctx.form_data()` (in `htmx-app` comments handler)
  - 2 `certify_all_assets` calls → `certify_assets` (in `react-app` and `htmx-app`)
  - Verified zero remaining verbose patterns in `examples/` and `tests/e2e/` via grep.

#### 7.7.8–7.7.9: Dependency and internal cleanup

- **7.7.8** Updated `ic-http-certification` and `ic-certification` in `Cargo.toml` from `"3.0.3"` to `"3.1"` to match actual resolved versions.
- **7.7.9** Evaluated internal `eq_ignore_ascii_case` usage in `src/lib.rs` and `src/asset_router.rs` — both operate on raw `HttpResponse`/`HttpRequest` headers (not `RouteContext`), so no migration applicable.

#### 7.7.10: syn-based source scanning

- **7.7.10** Replaced three string-based scanning functions in `src/build.rs` with `syn::parse_file`-based AST walking:
  - `scan_certification_attribute()` — walks `Item::Fn` attrs for `#[route(...)]` containing "certification"
  - `has_search_params()` — walks `Item::Struct` for `pub struct SearchParams`
  - `scan_route_attribute()` — walks `Item::Fn` attrs for `#[route(path = "...")]`
  - Added `syn = { version = "2", features = ["full", "parsing"] }` to `[dependencies]`
  - Added 5 new tests: multi-line `#[route(certification = ...)]`, comment-in-code false-positive rejection, multi-line struct detection, multi-line `#[route(path = ...)]`

#### 7.7.11: Final verification

- **7.7.11** Full verification passed:
  - `cargo check` — clean
  - `cargo test` — 283 unit tests pass, 10 doc tests pass
  - `cargo doc --no-deps` — builds without warnings

### Obstacles encountered

- **`syn` as dependency vs build-dependency**: The spec describes adding `syn` to `[build-dependencies]`, but `src/build.rs` is a library module (not a root `build.rs` file) consumed by downstream crates' build scripts. Therefore `syn` was added to `[dependencies]` instead of `[build-dependencies]`.
- **No `RouteContext`-based internal code to migrate**: Task 7.7.9 found that both remaining `eq_ignore_ascii_case` sites in `src/lib.rs` and `src/asset_router.rs` operate on raw `HttpResponse`/`HttpRequest` headers (not `RouteContext`), so no migration was needed. The spec anticipated this possibility.

### Out-of-scope observations

- All Phase 7 spec groups (7.1–7.7) are now complete. The implementation plan has zero remaining unchecked tasks.
- The `syn` dependency adds to the library's compile footprint (~2-3s for clean builds). This is acceptable since `syn` is already transitively compiled via the `macros/` crate, and the correctness improvement (multi-line attribute support, comment/string-literal immunity) justifies it.
- The `scan_pub_fns()` function (used by `detect_method_exports` and `has_pub_fn`) was not migrated to syn — it was not mentioned in the spec and its line-by-line scanning is adequate for `pub fn <name>(` detection. Consider migrating it in a future cleanup pass for full consistency.

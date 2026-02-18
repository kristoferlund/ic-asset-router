# Phase 8 — Code Quality & Robustness: Implementation Plan

**Scope:** Specs 8.1 through 8.6  
**Target codebase:** `/Users/kristoferlund/gh/ic-asset-router`  
**Status:** Not started

---

## Dependency Order

```
8.2  Eliminate Production Panics (independent — small, high impact)
 │
 ▼
8.1  Decompose http_request Functions (depends on nothing, but benefits from 8.2 being done first)
 │
 ├──▶ 8.3  Deduplicate Router and Asset Router Internals (benefits from 8.1 extraction)
 │
 ▼
8.4  Harden Build Script (independent)
 │
 ▼
8.5  Route Trie Optimization (depends on 8.3 for get_or_create_node)
 │
 ▼
8.6  Test Coverage and Edge Cases (depends on 8.1–8.5, tests the refactored code)
```

**Execution order:** 8.2 → 8.1 → 8.3 → 8.4 → 8.5 → 8.6

**Rationale:**
- 8.2 is the smallest spec with the highest production safety impact (eliminates panics and a memory leak). Do it first to reduce risk immediately.
- 8.1 is the largest refactor and touches the most code. Do it early so that subsequent specs work with the cleaned-up structure.
- 8.3 deduplicates internals in router.rs and asset_router.rs. The router dedup introduces `get_or_create_node` which 8.5 builds on.
- 8.4 is independent (build script only) and can run anytime, but placing it after the main refactors avoids merge conflicts in build.rs.
- 8.5 restructures the route trie and adds URL-decoding. It depends on 8.3's `get_or_create_node` extraction.
- 8.6 is last — it tests the final state of all refactored code.

**Note on refactoring:** All specs in this phase are pure refactors or safety fixes. No public API changes, no new features. Every task must preserve existing behavior — if `cargo test` regresses, the refactor is wrong.

---

## Tasks

### 8.2 — Eliminate Production Panics

- [x] **8.2.1** Replace `ic_cdk::trap` calls in `certify_dynamic_response_with_ttl` (`src/lib.rs`) with `debug_log!` + return the uncertified response. Three call sites: missing request for Full mode, `certify_dynamic_asset` error, `certify_asset` error.
- [x] **8.2.2** Fix `url_decode` memory leak in `src/context.rs:309`: replace `.leak()` with `.into_owned()` so `String::from_utf8_lossy` returns an owned `Cow` without permanent heap allocation.
- [x] **8.2.3** Audit all `.unwrap()`, `.expect()`, and `ic_cdk::trap()` calls in `src/` (excluding `#[cfg(test)]` blocks and `src/build.rs`). Document each as either (a) infallible and safe, or (b) needs fixing. Fix any that can fire in production.
- [x] **8.2.4** Add unit test in `src/context.rs`: `url_decode` with invalid UTF-8 byte sequences (e.g., `"%FF%FE"`) returns a valid string without panicking.
- [x] **8.2.5** Verify: `cargo check` and `cargo test` pass.

---

### 8.1 — Decompose http_request Functions

Depends on: 8.2

This is the largest spec. Split into two sub-sections: query path and update path.

#### 8.1A — Extract shared helpers and decompose http_request

- [ ] **8.1.1** Extract `is_asset_expired(asset, path, now_ns) -> bool` as a free function in `src/lib.rs`. Must check asset's own TTL first, then fall back to `ROUTER_CONFIG` global TTL. Replace all 4 inline copies (two in `http_request`, one in `http_request_update`, verify if a fourth exists).
- [ ] **8.1.2** Extract `attach_skip_certification(path: &str, response: &mut HttpResponse) -> Result<(), HttpResponse>` — the shared logic for adding CEL skip header, borrowing HTTP_TREE, getting data_certificate, building witness, and calling `add_v2_certificate_header`. Used by both the `certify=false` and `Skip` mode branches.
- [ ] **8.1.3** Extract `serve_uncertified(...)` from the `opts.certify == false` branch — runs handler via `execute_with_middleware`, then calls `attach_skip_certification`.
- [ ] **8.1.4** Extract `serve_skip_mode(...)` from the `Skip` certification mode branch — same pattern, delegates to `attach_skip_certification`.
- [ ] **8.1.5** Extract `serve_from_cache_or_upgrade(req, path)` from the cache-check + asset-router serve logic in the `certify == true` Found branch.

#### 8.1B — Decompose http_request_update and verify

- [ ] **8.1.6** Extract `handle_not_found_query(req, path, root, opts)` from the `NotFound` branch of `http_request`.
- [ ] **8.1.7** Extract `handle_not_modified(req, path)` from the `HandlerResult::NotModified` branch of `http_request_update`.
- [ ] **8.1.8** Extract `handle_not_found_update(req, path, root)` from the `NotFound` branch of `http_request_update`.
- [ ] **8.1.9** Verify: `http_request` top-level body is under 60 lines. `http_request_update` top-level body is under 60 lines. No extracted helper exceeds 80 lines.
- [ ] **8.1.10** Verify: `cargo check`, `cargo test`, and `cargo doc --no-deps` pass with no regressions or new warnings.

---

### 8.3 — Deduplicate Router and Asset Router Internals

Depends on: 8.1

- [ ] **8.3.1** Extract `get_or_create_node(&mut self, path: &str) -> &mut RouteNode` in `src/router.rs` — the shared trie traversal that parses path segments and creates intermediate nodes.
- [ ] **8.3.2** Rewrite `_insert` and `_insert_result` as thin wrappers (≤5 lines each) that call `get_or_create_node` then insert into the appropriate `HashMap`.
- [ ] **8.3.3** Add unit test for `get_or_create_node`: creates intermediate nodes on first call, returns same node on second call (idempotent), handles root path `/`.
- [ ] **8.3.4** Extract `certify_inner(...)` in `src/asset_router.rs` — the shared certification logic (build CEL expression, build response, create tree entry, insert into tree, construct CertifiedAsset, register fallbacks/aliases).
- [ ] **8.3.5** Rewrite `certify_asset` and `certify_dynamic_asset` as thin wrappers (≤15 lines each) that set up parameters then delegate to `certify_inner`.
- [ ] **8.3.6** Verify: `cargo check` and `cargo test` pass.

---

### 8.4 — Harden Build Script

Depends on: None (independent, but scheduled after 8.3 to avoid merge conflicts)

- [ ] **8.4.1** Rewrite `scan_pub_fns` in `src/build.rs` to use `syn::parse_file` + `Item::Fn` iteration instead of text-based line matching. Only detect functions with `syn::Visibility::Public`.
- [ ] **8.4.2** Add tests: `scan_pub_fns` ignores private functions (`fn get`), handles multi-line function signatures, handles functions with generics.
- [ ] **8.4.3** Fix `scan_route_attribute` in `src/build.rs`: replace the `tokens.find("path")` substring approach with proper `syn` meta parsing (walk `Meta::List` contents, match on exact ident `path`).
- [ ] **8.4.4** Add test: `scan_route_attribute` does not match a hypothetical attribute containing `xpath` or `mypath` as a value.
- [ ] **8.4.5** Replace all bare `.unwrap()` on filesystem operations in `src/build.rs` (`fs::read_dir`, `entry.unwrap()`, `file_name().unwrap()`, `to_str().unwrap()`) with `.unwrap_or_else(|e| panic!("...{path}...{e}"))` including the file/directory path in the message.
- [ ] **8.4.6** Verify: `cargo check` and `cargo test` pass.

---

### 8.5 — Route Trie Optimization

Depends on: 8.3 (uses `get_or_create_node`)

- [ ] **8.5.1** Replace `children: Vec<RouteNode>` in `src/router.rs` with three typed fields: `static_children: HashMap<String, RouteNode>`, `param_child: Option<Box<RouteNode>>`, `wildcard_child: Option<Box<RouteNode>>`.
- [ ] **8.5.2** Update all methods that access children: `get_or_create_node` (from 8.3), `_match`, `_resolve`, `skip_certified_paths`, `get_route_config`, `set_middleware`, `Display`/`Debug` impls, and any test helpers.
- [ ] **8.5.3** Add build-time `cargo:warning` diagnostics in `src/build.rs` for: (a) conflicting param directories at the same level (e.g., `_userId/` and `_postId/` as siblings), (b) `all.rs` in a directory that also has subdirectories (unreachable post-wildcard routes).
- [ ] **8.5.4** URL-decode route parameters in generated wrapper code (`src/build.rs`): apply `ic_asset_router::url_decode(...).into_owned()` when populating each `Params` struct field.
- [ ] **8.5.5** URL-decode wildcard value in generated `RouteContext` construction: `wildcard: raw_params.get("*").map(|w| ic_asset_router::url_decode(w).into_owned())`.
- [ ] **8.5.6** Add unit tests in `src/router.rs`: param with `%20` resolves and the raw param value contains `%20` (decoding happens in generated code, not in the trie). Add build-script-level test confirming generated code includes `url_decode`.
- [ ] **8.5.7** Update any existing tests that assert raw-encoded parameter values to expect decoded values where the generated wrapper is involved.
- [ ] **8.5.8** Verify: `cargo check`, `cargo test`, and `cargo doc --no-deps` pass.

---

### 8.6 — Test Coverage and Edge Cases

Depends on: 8.1, 8.2, 8.3, 8.4, 8.5

- [ ] **8.6.1** Add `url_decode` edge case tests in `src/context.rs`: trailing `%` (→ `"%"`), lone `%` at end of string, `%` followed by one valid hex char then EOF, `%00` (null byte), double-encoded `%2520` (→ `"%20"`, single decode only), empty string (→ `""`).
- [ ] **8.6.2** Add router trie edge case tests in `src/router.rs`: multiple param children (first wins or error), wildcard consumes all remaining segments (`/files/*` matches `/files/a/b/c`), empty path resolves to root, trailing slash normalization (`/about/` matches `/about`).
- [ ] **8.6.3** Add asset certification edge case tests in `src/asset_router.rs`: re-certifying same path replaces old entry, deleting nonexistent path is a no-op (no panic).
- [ ] **8.6.4** Add config header dedup tests in `src/config.rs`: later header overrides earlier with same key, override is case-insensitive (`Content-Type` overrides `content-type`).
- [ ] **8.6.5** Add `is_asset_expired` unit tests in `src/lib.rs` (after 8.1 extraction): asset with own TTL not expired, asset with own TTL expired, asset without TTL uses global config, asset without TTL and no global config never expires, static asset never expires.
- [ ] **8.6.6** Verify: `cargo test` passes, all new tests green.

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
cd tests/e2e && cargo test
```

---

## Session Boundaries

- Each session works on **one spec group only** (e.g., "8.2" or "8.1A+8.1B"), then stops.
- Spec 8.1 is large — it may be split across sessions by sub-section (8.1A, 8.1B), but all sub-sections must complete before moving to 8.3.
- If a verification step fails **twice in a row** after attempted fixes, mark the failing task with `[!]`, git commit all partial work, and stop.
- The agent **must git commit** all changes before stopping, every time.
- Do **not** continue to the next spec group after completing one — stop and let the user start a new session.

---

## Session Prompt Template

Paste this into a new agent session to execute the next spec group:

~~~
Read the implementation plan at:
  /Users/kristoferlund/gh/ic-asset-router/specs/Phase 8/PLAN.md

Find the first spec group that has incomplete tasks (unchecked `- [ ]` items).
Read the corresponding spec file in:
  /Users/kristoferlund/gh/ic-asset-router/specs/Phase 8/

Study the relevant source files in the target codebase at:
  /Users/kristoferlund/gh/ic-asset-router/

Read SESSION.md in the same directory as PLAN.md for notes from previous sessions.

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
8. APPEND a session summary to the END of SESSION.md (do NOT overwrite — read first, then add after the last line). Use heading `## Session N: Spec X.Y — <title>` (increment N). Include: what was accomplished, obstacles encountered, out-of-scope observations.
9. STOP. Do not continue to the next spec group.

IMPORTANT: All specs in this phase are pure refactors — no public API changes,
no new features. Every change must preserve existing behavior. If `cargo test`
regresses after a refactor, the refactor is wrong — fix the refactor, not the
test.
~~~

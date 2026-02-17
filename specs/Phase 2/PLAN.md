# Phase 2 — Implementation Plan

**Scope:** Specs 2.1 through 2.3
**Library:** `~/gh/router_library/`
**Status:** Not started

## Design Principles

- **Programmatic API is the foundation.** File-based routing is sugar — the code generator produces programmatic calls.
- **No backward compatibility** with the old `handler` convention. Clean break.
- **Consistent naming:** `router.set_*()` for all configuration (`set_middleware`, `set_not_found`).
- **One middleware per level.** No stacking.

## Dependency Order

```
2.1 (method dispatch)   — must go first; changes RouteNode, HandlerFn, build.rs
2.2 (middleware)         — depends on 2.1 (needs method-aware routing in place)
2.3 (custom 404)        — depends on 2.1 (uses method-specific exports); light dep on 2.2 (middleware chain must skip 404 handler)
```

Strict sequential order: 2.1 → 2.2 → 2.3.

---

## Tasks

### 2.1 — HTTP Method Dispatch

#### 2.1.1 — Router: method-aware handler storage

- [x] **2.1.1a** Change `HandlerFn` type or keep as-is (it's already `fn(HttpRequest, RouteParams) -> HttpResponse<'static'>`)
- [x] **2.1.1b** Replace `handler: Option<HandlerFn>` in `RouteNode` with `handlers: HashMap<Method, HandlerFn>` (import `Method` from `ic-http-certification`)
- [x] **2.1.1c** Update `RouteNode::new()` — initialize with empty `handlers` HashMap
- [x] **2.1.1d** Update `RouteNode::insert()` signature to accept a `Method` parameter: `pub fn insert(&mut self, path: &str, method: Method, handler: HandlerFn)`
- [x] **2.1.1e** Update `_insert()` to store handler in `handlers.insert(method, handler)` instead of `self.handler = Some(handler)`
- [x] **2.1.1f** Verify: `cargo check` passes (will break — tests and lib.rs still use old API; that's expected)

#### 2.1.2 — Router: method-aware lookup

- [x] **2.1.2a** Update `RouteNode::_match()` to return the full `handlers` HashMap (or the matched node) instead of a single handler
- [x] **2.1.2b** Create a new public method: `pub fn resolve(&self, path: &str, method: &Method) -> RouteResult` where `RouteResult` is an enum:
  ```rust
  pub enum RouteResult {
      Found(HandlerFn, RouteParams),
      MethodNotAllowed(Vec<Method>),  // path exists, wrong method
      NotFound,
  }
  ```
- [x] **2.1.2c** Implement `resolve`: match path → if node found, check `handlers` for method → if missing, return `MethodNotAllowed` with list of registered methods → if no node, return `NotFound`
- [x] **2.1.2d** Verify: `cargo check` passes (tests still broken, that's fine)

#### 2.1.3 — 405 Method Not Allowed response

- [x] **2.1.3a** Add helper function `method_not_allowed(allowed: &[Method]) -> HttpResponse<'static>` in `src/lib.rs` — returns 405 with `Allow` header and plain-text body
- [x] **2.1.3b** Verify: `cargo check` passes

#### 2.1.4 — Integration: lib.rs

- [x] **2.1.4a** Update `http_request()` in `src/lib.rs`: extract method from request, call `router.resolve(path, method)`, handle all three `RouteResult` variants
- [x] **2.1.4b** Update `http_request_update()` similarly
- [x] **2.1.4c** Ensure non-GET requests arriving at `http_request` (query) trigger `upgrade = true` so they flow to `http_request_update`
- [x] **2.1.4d** Verify: `cargo check` passes

#### 2.1.5 — Build script: method detection

- [x] **2.1.5a** Update `build.rs` to scan route files for exported function names matching: `get`, `post`, `put`, `patch`, `delete`, `head`, `options`
- [x] **2.1.5b** For each detected method function, generate a `root.insert(path, Method::GET, routes::path::get)` call (etc.)
- [x] **2.1.5c** Remove generation of the old `handler` function references
- [x] **2.1.5d** Add compile-time error (via `compile_error!` or `panic!` in build.rs) if a route file exports no recognized method functions
- [x] **2.1.5e** Verify: `cargo check` passes

#### 2.1.6 — Update existing route files

- [x] **2.1.6a** Rename `handler` to `get` (or appropriate method) in all existing route files under `src/routes/` in the library and any example/test routes
- [x] **2.1.6b** Verify: `cargo check` passes
- [x] **2.1.6c** Verify: `cargo test` passes

#### 2.1.7 — Tests

- [x] **2.1.7a** Add test: `GET /path` routes to `get` handler, `POST /path` routes to `post` handler
- [x] **2.1.7b** Add test: `PUT /path` returns 405 with `Allow: GET, POST` when only `get` and `post` are registered
- [x] **2.1.7c** Add test: unknown path returns `NotFound`
- [x] **2.1.7d** Add test: all 7 method types can be registered and resolved
- [x] **2.1.7e** Update any existing router tests broken by the API change
- [x] **2.1.7f** Verify: `cargo test` passes

#### 2.1.8 — Commit

- [x] **2.1.8a** `git add -A && git commit -m "spec 2.1: HTTP method dispatch"`

---

### 2.2 — Middleware System

#### 2.2.1 — Middleware types

- [x] **2.2.1a** Create `src/middleware.rs` with:
  ```rust
  pub type MiddlewareFn = fn(
      req: HttpRequest,
      params: &RouteParams,
      next: &dyn Fn(HttpRequest, &RouteParams) -> HttpResponse<'static>,
  ) -> HttpResponse<'static>;
  ```
- [x] **2.2.1b** Export from `src/lib.rs`
- [x] **2.2.1c** Verify: `cargo check` passes

#### 2.2.2 — Router: middleware storage

- [x] **2.2.2a** Add middleware storage to the router (or a separate struct): a `Vec<(String, MiddlewareFn)>` storing (prefix, middleware) pairs, sorted by prefix depth (shortest first)
- [x] **2.2.2b** Implement `router.set_middleware(prefix: &str, middleware: MiddlewareFn)` — replaces any existing middleware at that prefix
- [x] **2.2.2c** Verify: `cargo check` passes

#### 2.2.3 — Middleware chain execution

- [x] **2.2.3a** Implement `build_chain()`: given a request path, collect all middleware whose prefix matches (sorted outer → inner), then wrap the handler as the innermost `next`
- [x] **2.2.3b** The chain nests: each middleware's `next` calls the next middleware inward, with the handler at the center
- [x] **2.2.3c** Verify: `cargo check` passes

#### 2.2.4 — Integration: lib.rs

- [x] **2.2.4a** Update `http_request()`: after resolving route, execute middleware chain instead of calling handler directly
- [x] **2.2.4b** Update `http_request_update()` similarly
- [x] **2.2.4c** Verify: `cargo check` passes

#### 2.2.5 — Build script: middleware detection

- [x] **2.2.5a** Update `build.rs` to detect `middleware.rs` files in route directories
- [x] **2.2.5b** Generate `router.set_middleware("/prefix", routes::prefix::middleware::middleware)` calls for each detected `middleware.rs`
- [x] **2.2.5c** Root `middleware.rs` maps to `router.set_middleware("/", ...)`
- [x] **2.2.5d** Verify: `cargo check` passes

#### 2.2.6 — Tests

- [x] **2.2.6a** Add test: root middleware runs on all requests
- [x] **2.2.6b** Add test: scoped middleware runs only on matching prefix
- [x] **2.2.6c** Add test: chain order is root → outer → inner → handler → inner → outer → root (use a side-effect log to verify order)
- [x] **2.2.6d** Add test: middleware can short-circuit (return without calling next)
- [x] **2.2.6e** Add test: middleware can modify the response from next
- [x] **2.2.6f** Add test: `set_middleware` on same prefix replaces previous middleware
- [x] **2.2.6g** Add test: middleware works in both query and update paths
- [x] **2.2.6h** Verify: `cargo test` passes

#### 2.2.7 — Commit

- [x] **2.2.7a** `git add -A && git commit -m "spec 2.2: middleware system"`

---

### 2.3 — Custom 404 Response

#### 2.3.1 — Router: not-found handler storage

- [x] **2.3.1a** Add `not_found_handler: Option<HandlerFn>` to the router (or `HashMap<Method, HandlerFn>` for method-specific 404s)
- [x] **2.3.1b** Implement `router.set_not_found(handler: HandlerFn)` — single handler for all methods
- [x] **2.3.1c** Verify: `cargo check` passes

#### 2.3.2 — Integration: lib.rs

- [x] **2.3.2a** Update the `NotFound` branch in `http_request()`: if custom not-found handler is registered, call it (passing request with empty params); otherwise return default 404
- [x] **2.3.2b** Same for `http_request_update()`
- [x] **2.3.2c** Ensure the middleware chain also runs for not-found requests (root/global middleware should still execute before the 404 handler)
- [x] **2.3.2d** Verify: `cargo check` passes

#### 2.3.3 — Build script: not_found.rs detection

- [x] **2.3.3a** Update `build.rs` to detect `not_found.rs` in the routes root directory
- [x] **2.3.3b** Generate `router.set_not_found(routes::not_found::get)` (or method-specific registration if multiple methods exported)
- [x] **2.3.3c** Verify: `cargo check` passes

#### 2.3.4 — Tests

- [x] **2.3.4a** Add test: with custom 404, unmatched route returns custom response
- [x] **2.3.4b** Add test: without custom 404, unmatched route returns default "Not Found"
- [x] **2.3.4c** Add test: custom 404 handler receives the full HttpRequest
- [x] **2.3.4d** Add test: custom 404 can return JSON content-type
- [x] **2.3.4e** Add test: root middleware executes before custom 404 handler
- [x] **2.3.4f** Verify: `cargo test` passes

#### 2.3.5 — Commit

- [x] **2.3.5a** `git add -A && git commit -m "spec 2.3: custom 404 response"`

---

## Verification Protocol

After each spec is complete, run:

```
cargo check
cargo test
```

Both must pass before marking the spec as done.

After all Phase 2 specs are complete, run a final check:

```
cargo check
cargo test
cargo doc --no-deps
```

---

## Session Boundaries

Each OpenCode session works on **one spec** (e.g., all tasks under 2.1). When all tasks for that spec are checked off and verified, the session ends. The next session picks up the next spec.

**Why one spec per session:**
- Context stays focused on one concern
- No context pollution from unrelated file reads and failed attempts
- The plan file carries state between sessions — nothing is lost
- If the agent degrades mid-spec, you lose at most one spec's worth of work

**When to stop early (before completing the spec):**
- After a failed verification (`cargo check` or `cargo test` fails) and one fix attempt also fails — stop, start fresh
- If the agent starts producing low-quality output — stop. Context is degraded.

**After each session:**
- The agent must have committed its work before stopping
- Verify the plan file was updated (tasks checked off)

## Session Prompt Template

At the start of each OpenCode session, use this prompt:

```
You are implementing changes to a Rust library at ~/gh/router_library/.

PLAN FILE: ~/Documents/BEE NOTES/Projects/Asset Router/Phase 2/PLAN.md
SPEC FILES: ~/Documents/BEE NOTES/Projects/Asset Router/Phase 2/

Instructions:
1. Read the plan file (PLAN.md).
2. Find the next incomplete spec (the first spec group with unchecked tasks).
3. Read the corresponding spec file for that spec (e.g., "2.1 — HTTP Method Dispatch.md").
4. Study the library source files relevant to that spec.
5. Implement each unchecked task in that spec group, in order.
6. After each task: run `cargo check`. If it fails, fix before continuing.
7. After completing all tasks in the spec group: run `cargo test` and `cargo doc`. Fix any failures.
8. Mark completed tasks in PLAN.md (change `[ ]` to `[x]`).
9. Commit all changes: `git add -A && git commit -m "spec X.X: <brief description>"`
10. STOP after completing one spec group. Do not continue to the next spec.

Rules:
- Do not skip tasks. Do not reorder tasks within a spec group.
- If `cargo check` or `cargo test` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Ask before installing new dependencies.
- Do not modify files outside ~/gh/router_library/ except for PLAN.md.
- ALWAYS commit before stopping. Every session must end with a git commit.
```

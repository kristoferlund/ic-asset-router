# Phase 3 — Implementation Plan

**Scope:** Spec 3.1 (Template Engine & HTMX Examples)
**Library:** `~/gh/router_library/`
**Status:** Complete (Session 1: tasks 3.1.1–3.1.11, Session 2: tasks 3.1.12–3.1.28)

## Dependency Order

```
3.1 (Template Engine & HTMX Examples) — depends on Phase 1.2 (handler-controlled content-type)
```

Phase 3 has a single spec. It is primarily documentation and example code, not library changes. The prerequisite is that spec 1.2 is complete (handlers set content-type, library respects it through certification). Based on the current codebase state (config.rs, middleware.rs exist; Phase 1 work is done), this prerequisite is satisfied.

Execution within 3.1 is ordered: documentation first (Askama pattern, Tera pattern, comparison), then the HTMX example canister (which is the larger deliverable).

---

## Tasks

### 3.1 — Template Engine & HTMX Examples

#### Documentation: Askama Integration

- [x] **3.1.1** Create `examples/` directory in the library repo
- [x] **3.1.2** Create `examples/askama-basic/` with a minimal Cargo.toml that depends on `router_library` (path dependency), `askama`, and the IC crates
- [x] **3.1.3** Create `examples/askama-basic/templates/post.html` — a simple Askama template with `{{ title }}`, `{{ content }}`, `{{ author }}` variables
- [x] **3.1.4** Create `examples/askama-basic/src/lib.rs` — a minimal canister with one route (`/posts/:postId`) that renders the Askama template and returns HTML with correct content-type header
- [x] **3.1.5** Verify: `cargo check` succeeds for `examples/askama-basic/` (use `--manifest-path`)

#### Documentation: Tera Integration

- [x] **3.1.6** Create `examples/tera-basic/` with a minimal Cargo.toml that depends on `router_library` (path dependency), `tera`, and the IC crates
- [x] **3.1.7** Create `examples/tera-basic/templates/post.html` — a simple Tera template with `{{ title }}`, `{{ content }}` variables
- [x] **3.1.8** Create `examples/tera-basic/src/lib.rs` — a minimal canister with `thread_local!` Tera instance, `include_str!` template loading, one route that renders and returns HTML
- [x] **3.1.9** Verify: `cargo check` succeeds for `examples/tera-basic/` (use `--manifest-path`)

#### Documentation: Comparison and README

- [x] **3.1.10** Add a template engine integration section to the library's `README.md` covering: Askama pattern summary, Tera pattern summary, Askama vs Tera comparison table, links to the example directories
- [x] **3.1.11** Verify: documentation references correct file paths and the examples exist

#### HTMX Example Canister: Project Scaffold

- [x] **3.1.12** Create `examples/htmx-app/Cargo.toml` — depends on `router_library` (path dependency), `askama`, `include_dir`, and the IC crates (`ic-cdk`, `ic-http-certification`, `ic-asset-certification`)
- [x] **3.1.13** Create `examples/htmx-app/dfx.json` — ICP project configuration with the canister defined as a Rust canister
- [x] **3.1.14** Create `examples/htmx-app/static/style.css` — minimal CSS for the example (basic typography, layout)
- [x] **3.1.15** Download or create a minimal `examples/htmx-app/static/htmx.min.js` — the HTMX library (or reference a CDN URL in the HTML and skip the local file)

#### HTMX Example Canister: Templates

- [x] **3.1.16** Create `examples/htmx-app/templates/layout.html` — base Askama layout with `<head>` (includes htmx.min.js, style.css), `<nav>`, `{% block content %}`, `<footer>`
- [x] **3.1.17** Create `examples/htmx-app/templates/index.html` — extends layout, renders a list of posts with links
- [x] **3.1.18** Create `examples/htmx-app/templates/post.html` — extends layout, renders a single post with title, content, author, and an HTMX-powered comments section
- [x] **3.1.19** Create `examples/htmx-app/templates/partials/comments.html` — HTML fragment (no layout), renders a list of comments; this is returned by the HTMX partial endpoint

#### HTMX Example Canister: Routes

- [x] **3.1.20** Create `examples/htmx-app/src/routes/index.rs` — handler for `GET /` that renders the index template with a hardcoded list of posts
- [x] **3.1.21** Create `examples/htmx-app/src/routes/posts/:postId/index.rs` — handler for `GET /posts/:postId` that renders the post template with hardcoded post data (uses directory+index pattern to avoid build.rs param-directory bug)
- [x] **3.1.22** Create `examples/htmx-app/src/routes/posts/:postId/comments.rs` — handler for `GET /posts/:postId/comments` that returns the comments partial (HTML fragment, no layout wrapper)

#### HTMX Example Canister: Canister Entry Point

- [x] **3.1.23** Create `examples/htmx-app/src/lib.rs` — canister entry point with `#[init]`, `#[post_upgrade]` that certifies static assets and registers routes; `#[query] http_request` and `#[update] http_request_update` that delegate to the library
- [x] **3.1.24** Create `examples/htmx-app/src/data.rs` — hardcoded sample data (a few posts with titles, content, authors, comments) used by the route handlers

#### HTMX Example Canister: Build Script

- [x] **3.1.25** Create `examples/htmx-app/build.rs` — calls `router_library::build::generate_routes()` to generate the route tree from the file-based routes (includes workaround patching mod.rs for param directories)

#### Verification

- [x] **3.1.26** Verify: `cargo check --manifest-path examples/htmx-app/Cargo.toml` succeeds
- [x] **3.1.27** Verify: `cargo build --manifest-path examples/htmx-app/Cargo.toml --target wasm32-unknown-unknown` succeeds
- [x] **3.1.28** Verify: the example demonstrates all three patterns — full page routes, HTMX partial routes, and dynamic route parameters

---

## Verification Protocol

After spec 3.1 is complete, run:

```
cargo check --manifest-path examples/askama-basic/Cargo.toml
cargo check --manifest-path examples/tera-basic/Cargo.toml
cargo check --manifest-path examples/htmx-app/Cargo.toml
```

All must succeed. The main library's tests should also still pass:

```
cargo test
```

If the `wasm32-unknown-unknown` target is installed, also verify:

```
cargo build --manifest-path examples/htmx-app/Cargo.toml --target wasm32-unknown-unknown
```

---

## Session Boundaries

Phase 3 has a single spec (3.1) but it contains 28 tasks. Given the volume, it is reasonable to split across two sessions:

- **Session 1:** Tasks 3.1.1–3.1.11 (documentation examples: askama-basic, tera-basic, README section)
- **Session 2:** Tasks 3.1.12–3.1.28 (HTMX example canister)

Each session works on its portion, commits, and stops. If all 28 tasks fit comfortably in one session without context degradation, that's fine too.

**When to stop early:**
- After a failed verification (`cargo check` fails) and one fix attempt also fails — stop, start fresh
- If the agent starts producing low-quality output — stop, context is degraded

**After each session:**
- The agent must have committed its work before stopping
- Verify the plan file was updated (tasks checked off)

## Session Prompt Template

At the start of each OpenCode session, use this prompt:

```
You are implementing examples and documentation for a Rust library at ~/gh/router_library/.

PLAN FILE: ~/bee/BEE NOTES/Projects/Asset Router/Phase 3/PLAN.md
SPEC FILES: ~/bee/BEE NOTES/Projects/Asset Router/Phase 3/

Instructions:
1. Read the plan file (PLAN.md).
2. Find the next incomplete task group (documentation examples OR HTMX canister).
3. Read the spec file "3.1 — Template Engine & HTMX Examples.md" for full context.
4. Study the library source at ~/gh/router_library/ to understand current types, patterns, and APIs.
5. Implement each unchecked task in order.
6. After each major group (askama-basic, tera-basic, htmx-app): run `cargo check` with the appropriate --manifest-path. If it fails, fix before continuing.
7. After completing the task group: run the verification commands from the plan.
8. Mark completed tasks in PLAN.md (change `[ ]` to `[x]`).
9. Append a session summary to SESSION.md (in the same directory as PLAN.md).
   The summary must include:
   - A heading with the session name (e.g., "## Session 1: Spec 3.1 — Documentation Examples")
   - Date
   - What was accomplished (tasks completed, brief description)
   - Obstacles encountered (compilation errors, test failures, unclear specs, workarounds applied)
   - Out-of-scope observations: anything noticed during implementation that should be
     addressed elsewhere in the codebase but was not part of this session's tasks.
     These may become new specs or tasks in future phases.
   Create the file if it does not exist. Append to it if it does.
10. Commit all changes: `git add -A && git commit -m "phase 3.1: <brief description>"`
11. STOP after completing the current task group. Do not continue to the next group if session boundaries are defined.

Rules:
- Do not skip tasks. Do not reorder tasks.
- If `cargo check` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Ask before installing new dependencies in the main library. Example crates can add their own dependencies freely.
- Do not modify library source files (src/*.rs) — this phase is examples and documentation only.
- The only files outside ~/gh/router_library/ that you may modify are PLAN.md and SESSION.md.
- ALWAYS commit before stopping. Every session must end with a git commit.
```

You are implementing changes to a Rust library at ~/gh/router_library/.

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
9. Commit all changes: `git add -A && git commit -m "phase 3.1: <brief description>"`
10. STOP after completing the current task group. Do not continue to the next group if session boundaries are defined.

Rules:
- Do not skip tasks. Do not reorder tasks.
- If `cargo check` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Ask before installing new dependencies in the main library. Example crates can add their own dependencies freely.
- Do not modify library source files (src/*.rs) — this phase is examples and documentation only.
- The only files outside ~/gh/router_library/ that you may modify is PLAN.md.
- ALWAYS commit before stopping. Every session must end with a git commit.

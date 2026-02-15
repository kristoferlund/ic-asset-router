You are implementing changes to a Rust library at ~/gh/router_library/.

PLAN FILE: ~/bee/BEE NOTES/Projects/Asset Router/Phase 4/PLAN.md
SPEC FILES: ~/bee/BEE NOTES/Projects/Asset Router/Phase 4/

Instructions:
1. Read the plan file (PLAN.md).
2. Find the next incomplete spec (the first spec group with unchecked tasks).
3. Read the corresponding spec file for that spec (e.g., "4.2 — Explicit Invalidation API.md").
4. Study the library source files relevant to that spec.
5. Implement each unchecked task in that spec group, in order.
6. After each task: run `cargo check`. If it fails, fix before continuing.
7. After completing all tasks in the spec group: run `cargo test`. Fix any failures.
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


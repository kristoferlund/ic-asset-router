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
9. Commit all changes using the commit message specified at the end of the session.
10. STOP. Do not continue to the next session.

Rules:
- Do not skip tasks. Do not reorder tasks within a session.
- If `cargo check` or `cargo test` fails and your fix attempt also fails, STOP.
  Mark the failing task with `[!]` in PLAN.md and describe the failure briefly.
  Still commit whatever partial work exists before stopping.
- Ask before installing new dependencies.
- Do not modify files outside ~/gh/router_library/ except for PLAN.md.
- ALWAYS commit before stopping. Every session must end with a git commit.

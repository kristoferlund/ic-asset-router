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
- Ask before installing new dependencies.
- Do not modify files outside ~/gh/router_library/ except for PLAN.md and SESSION.md.
- ALWAYS commit before stopping. Every session must end with a git commit.

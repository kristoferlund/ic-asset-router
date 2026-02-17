# Building ic-asset-router with the RALPH Loop

This library was built almost entirely by an AI coding agent using the **RALPH loop** technique — a method for producing high-quality AI-generated code by keeping the context window focused on one task at a time.

## What is a RALPH loop?

The idea, introduced by [Geoffrey Huntley](https://ghuntley.com/loop), is straightforward: instead of dumping an entire project into a single sprawling AI session, you divide the work into small, well-specified tasks and execute them in a loop. Each iteration gets a clean context window with only the relevant spec, so the model operates at peak quality rather than degrading as context fills up.

[![Watch: Everything is a RALPH loop](https://img.youtube.com/vi/4Nna09dG_c0/maxresdefault.jpg)](https://youtu.be/4Nna09dG_c0)

## The workflow

### 1. Write detailed specs

Each feature is defined as a self-contained markdown file with clear acceptance criteria. Browse the [specs/](specs/README.md) folder to see every spec used to build this library.

### 2. Generate an implementation plan

A [reusable prompt](specs/Generate%20Implementation%20Plan%20—%20Prompt.md) is fed to the AI agent along with the folder of specs for one phase. The agent reads every spec, analyzes dependencies between them, and produces a `PLAN.md` with:

- A dependency graph determining execution order
- Atomic, checkboxed tasks decomposed from each spec
- A verification protocol (which commands to run after each task)
- A **session prompt template** — a ready-to-paste prompt that the loop script feeds to the agent in each iteration

The plan also defines session boundaries: one spec group per session, commit before stopping, never continue to the next group.

### 3. Loop

The `loop.sh` script automates execution. It pipes the session prompt to the coding agent, pushes the result, and repeats.

```
./loop.sh PROMPT.md 5    # Run 5 iterations
```

Each iteration: the agent reads `PLAN.md`, finds the next incomplete spec group, reads the corresponding spec, implements the tasks, runs verification, marks tasks complete, commits, and stops.

### 4. Session feedback (SESSION.md)

Starting from Phase 5, each session appends a summary to `SESSION.md` in the phase folder. The agent is instructed to record:

- What was accomplished
- Obstacles encountered (compilation errors, test failures, workarounds)
- **Out-of-scope observations** — anything noticed during implementation that should be addressed but wasn't part of the current task

This last point turned out to be the most valuable part of the process. The agent acts as a second pair of eyes on the codebase, identifying bugs, design gaps, and potential improvements as it works through each spec.

For example, during Phase 5 sessions the agent flagged issues like shared scanning logic that should be unified, lifetime warnings in test helpers, and generated files that should be gitignored. These observations became the raw material for Phase 6 — but they didn't turn into tasks automatically. The human still reviews the observations, decides which ones matter, writes proper specs with acceptance criteria, and feeds those specs through the plan generation step before the agent works on them. The agent surfaces problems; the human decides what to do about them.

See [Phase 5 SESSION.md](specs/Phase%205/SESSION.md) and [Phase 6 SESSION.md](specs/Phase%206/SESSION.md) for the full session logs.

## Phases

The library was developed across six phases. Each phase has a plan and a set of individual spec documents. See the [full specs](specs/README.md) for details.

| Phase | Focus | Items |
|-------|-------|-------|
| [Phase 1](specs/Phase%201/PLAN.md) | **Fix Foundations** — Configurable headers, error handling, cache control, wildcard capture | 7 |
| [Phase 2](specs/Phase%202/PLAN.md) | **Method Routing & Middleware** — HTTP method dispatch, scoped middleware, custom 404 | 3 |
| [Phase 3](specs/Phase%203/PLAN.md) | **SSR & Template Integration** — Askama/Tera examples, HTMX app with static assets | 1 |
| [Phase 4](specs/Phase%204/PLAN.md) | **Dynamic Asset Lifecycle** — TTL-based cache invalidation, explicit invalidation API, conditional regeneration | 3 |
| [Phase 5](specs/Phase%205/PLAN.md) | **Developer Experience** — Build script code generation, type-safe route params, typed search params, comprehensive test suite, PocketIC E2E tests | 7 |
| [Phase 6](specs/Phase%206/PLAN.md) | **Fixes & Polish** — Single certified 404 fallback, wildcard in RouteContext, warning suppression, example migration, E2E hardening | 9 |

## Why this approach works

- **Clean context** — Each session starts fresh with only the relevant spec, avoiding the quality degradation that comes from overloaded context windows.
- **Verifiable progress** — Every loop iteration produces a commit, so you can review changes incrementally.
- **Self-improving** — The session feedback mechanism means the agent surfaces bugs and design gaps as it works. These observations inform (but don't automatically become) the specs for later phases — the human still curates and specifies the work.
- **Reproducible** — The specs serve as documentation after the fact. Anyone can read them to understand not just what was built, but the reasoning behind each decision.

## Further reading

- [Everything is a RALPH loop](https://ghuntley.com/loop) — Geoffrey Huntley's post on the technique
- [Generate Implementation Plan — Prompt](specs/Generate%20Implementation%20Plan%20—%20Prompt.md) — The reusable prompt for converting specs into a plan
- [specs/](specs/README.md) — The full specification documents used to build this library

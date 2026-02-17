# Building ic-asset-router with the RALPH Loop

This library was built almost entirely by an AI coding agent using the **RALPH loop** technique — a method for producing high-quality AI-generated code by keeping the context window focused on one task at a time.

## What is a RALPH loop?

The idea, introduced by [Geoffrey Huntley](https://ghuntley.com/loop), is straightforward: instead of dumping an entire project into a single sprawling AI session, you divide the work into small, well-specified tasks and execute them in a loop. Each iteration gets a clean context window with only the relevant spec, so the model operates at peak quality rather than degrading as context fills up.

[![Watch: Everything is a RALPH loop](https://img.youtube.com/vi/4Nna09dG_c0/maxresdefault.jpg)](https://youtu.be/4Nna09dG_c0)

The process:

1. **Write detailed specs** — Define every feature as a self-contained specification with clear acceptance criteria.
2. **Create an implementation plan** — Group the specs into phases with dependency ordering.
3. **Loop** — Feed each spec to the AI agent one at a time via a shell script (`loop.sh`), pushing results after each iteration.

The `loop.sh` script in this repo automates step 3: it pipes a prompt file to the coding agent, pushes the result, and repeats.

```
./loop.sh PROMPT.md 5    # Run 5 iterations
```

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
- **Reproducible** — The specs serve as documentation after the fact. Anyone can read them to understand not just what was built, but the reasoning behind each decision.

## Further reading

- [Everything is a RALPH loop](https://ghuntley.com/loop) — Geoffrey Huntley's original post on the technique
- [specs/](specs/README.md) — The full specification documents used to build this library

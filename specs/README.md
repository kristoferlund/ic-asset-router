# ICP Asset Router — Project Overview

A Rust-based HTTP routing library for Internet Computer (ICP) canisters. It serves both **static and dynamic assets** with ICP response certification, using a file-based routing convention similar to Next.js and TanStack Router — but for Rust canisters.

## What It Does

- **Trie-based router** with static segments, named parameters (`:param`), and wildcards (`*`)
- **Two-phase HTTP model**: query calls serve pre-certified assets instantly; uncertified paths trigger update calls that generate, certify, and cache the response
- **Build-time code generator** maps a `src/routes/` filesystem convention to a route tree
- **Compile-time static asset embedding** via `include_dir` with automatic MIME detection

## Use Cases

1. **SPA with dynamic meta tags** — React/Svelte app where `index.html` needs per-route `<meta>` tags for social media previews and SEO
2. **Full server-side rendering** — HTMX-style apps, all HTML generated on the canister
3. **API endpoints** — JSON/REST APIs co-hosted with a frontend in a single canister (limited by ICP's lack of authenticated HTTP — there's no way to tie HTTP requests to ICP principals without custom signed-header schemes, since the standard principal-based authentication only works over the Candid API, not HTTP)

## Current State

The library works but has several issues that need fixing before production use. The most critical: hardcoded `text/html` content-type for all dynamic assets, hardcoded security headers with no override mechanism, and `unwrap()` calls that can trap the canister.

## Repos

- **Library**: [kristoferlund/ic-asset-router](https://github.com/kristoferlund/ic-asset-router)
- **Example project**: [kristoferlund/promptathon-showcase](https://github.com/kristoferlund/promptathon-showcase)

## Evolution Roadmap — 8 Phases, 48 Work Items

### Phase 1 — Fix Foundations *(breaking changes OK)*
Make the library correct and production-ready.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 1.1 | [Configurable Headers](Phase%201/1.1%20—%20Configurable%20Headers.md) | Critical | Medium |
| 1.2 | [Handler-Controlled Response Metadata](Phase%201/1.2%20—%20Handler-Controlled%20Response%20Metadata.md) | Critical | Small |
| 1.3 | [Graceful Error Handling](Phase%201/1.3%20—%20Graceful%20Error%20Handling.md) | Critical | Small |
| 1.4 | [Remove Debug Logging](Phase%201/1.4%20—%20Remove%20Debug%20Logging.md) | Low | Small |
| 1.5 | [Wildcard Segment Capture](Phase%201/1.5%20—%20Wildcard%20Segment%20Capture.md) | Medium | Small |
| 1.6 | [Minor Fixes](Phase%201/1.6%20—%20Minor%20Fixes.md) | Low | Small |
| 1.7 | [Configurable Cache-Control](Phase%201/1.7%20—%20Configurable%20Cache-Control.md) | Medium | Small |

Implementation plan: [Phase 1 Plan](Phase%201/PLAN.md)

### Phase 2 — Method Routing & Middleware
Support real API use cases and reduce handler boilerplate.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 2.1 | [HTTP Method Dispatch](Phase%202/2.1%20—%20HTTP%20Method%20Dispatch.md) | High | Medium |
| 2.2 | [Middleware System](Phase%202/2.2%20—%20Middleware%20System.md) | High | Large |
| 2.3 | [Custom 404 Response](Phase%202/2.3%20—%20Custom%20404%20Response.md) | Medium | Small |

Implementation plan: [Phase 2 Plan](Phase%202/PLAN.md)

### Phase 3 — SSR & Template Integration
Documentation and examples for the server-side rendering use case.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 3.1 | [Template Engine & HTMX Examples](Phase%203/3.1%20—%20Template%20Engine%20&%20HTMX%20Examples.md) | Medium | Medium |

Implementation plan: [Phase 3 Plan](Phase%203/PLAN.md)

### Phase 4 — Dynamic Asset Lifecycle
Solve the stale-cache problem for dynamically generated content.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 4.1 | [TTL-Based Cache Invalidation](Phase%204/4.1%20—%20TTL-Based%20Cache%20Invalidation.md) | Medium | Medium |
| 4.2 | [Explicit Invalidation API](Phase%204/4.2%20—%20Explicit%20Invalidation%20API.md) | High | Medium |
| 4.3 | [Conditional Regeneration](Phase%204/4.3%20—%20Conditional%20Regeneration.md) | Low | Small |

Implementation plan: [Phase 4 Plan](Phase%204/PLAN.md)

### Phase 5 — Developer Experience & Tooling
Make the library pleasant to use and hard to misuse.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 5.1 | [Build Script & IDE Ergonomics](Phase%205/5.1%20—%20Build%20Script%20&%20IDE%20Ergonomics.md) | High | Medium |
| 5.2 | [Type-Safe Route Params](Phase%205/5.2%20—%20Type-Safe%20Route%20Params.md) | High | Large |
| 5.3 | [Query String Access](Phase%205/5.3%20—%20Query%20String%20Access%20&%20Typed%20Search%20Params.md) | Medium | Small |
| 5.4 | [Reserved Filename Validation](Phase%205/5.4%20—%20Reserved%20Filename%20Validation.md) | Medium | Small |
| 5.5 | [Comprehensive Test Suite](Phase%205/5.5%20—%20Comprehensive%20Test%20Suite.md) | High | Medium |
| 5.6 | [Documentation & Examples](Phase%205/5.6%20—%20Documentation%20&%20Examples.md) | High | Large |
| 5.7 | [PocketIC E2E Tests](Phase%205/5.7%20—%20PocketIC%20End-to-End%20Tests.md) | High | Large |

Implementation plan: [Phase 5 Plan](Phase%205/PLAN.md)

### Phase 6 — Fixes & Polish *(from Phase 5 session findings)*
Address bugs, design gaps, and maintenance issues discovered during Phase 5 implementation.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 6.1 | [Not-Found Response Certification](Phase%206/6.1%20—%20Not-Found%20Response%20Certification.md) | High | Small |
| 6.2 | [Wildcard Value in RouteContext](Phase%206/6.2%20—%20Wildcard%20Value%20in%20RouteContext.md) | High | Small |
| 6.3 | [Generated Code Warning Suppression](Phase%206/6.3%20—%20Generated%20Code%20Warning%20Suppression.md) | Medium | Small |
| 6.4 | [Example Migration](Phase%206/6.4%20—%20Example%20Migration.md) | Medium | Medium |
| 6.5 | [E2E Test Hardening](Phase%206/6.5%20—%20E2E%20Test%20Hardening.md) | Medium | Medium |
| 6.6 | [Minor Fixes & Cleanup](Phase%206/6.6%20—%20Minor%20Fixes%20&%20Cleanup.md) | Low | Small |
| 6.7 | [Single Certified 404 Fallback](Phase%206/6.7%20—%20Single%20Certified%20404%20Fallback.md) | High | Medium |
| 6.8 | [Expose URL Decode & Form Parse Utilities](Phase%206/6.8%20—%20Expose%20URL%20Decode%20&%20Form%20Parse%20Utilities.md) | Medium | Small |
| 6.9 | [Fix Fragile Template Paths](Phase%206/6.9%20—%20Fix%20Fragile%20Template%20Paths%20in%20Examples.md) | Low | Small |

Implementation plan: [Phase 6 Plan](Phase%206/PLAN.md)

### Phase 7 — Configurable Certification Modes
Per-route and per-asset certification mode configuration with a proc-macro attribute.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 7.1 | [Define Certification Configuration Types](Phase%207/7.1%20—%20Define%20Certification%20Configuration%20Types.md) | Critical | Medium |
| 7.2 | [Build Asset Router](Phase%207/7.2%20—%20Build%20Asset%20Router.md) | Critical | Large |
| 7.3 | [Refactor certify_assets for Certification Modes](Phase%207/7.3%20—%20Refactor%20certify_assets%20for%20Certification%20Modes.md) | High | Medium |
| 7.4 | [Per-Route Certification Configuration](Phase%207/7.4%20—%20Per-Route%20Certification%20Configuration.md) | High | Large |
| 7.5 | [Documentation and Examples for Certification Modes](Phase%207/7.5%20—%20Documentation%20and%20Examples%20for%20Certification%20Modes.md) | Medium | Medium |
| 7.6 | [Integration Tests for Certification Modes](Phase%207/7.6%20—%20Integration%20Tests%20for%20Certification%20Modes.md) | High | Medium |
| 7.7 | [RouteContext Ergonomic Improvements](Phase%207/7.7%20—%20RouteContext%20Ergonomic%20Improvements.md) | Medium | Small |

Implementation plan: [Phase 7 Plan](Phase%207/PLAN.md)

### Phase 8 — Code Quality & Robustness
Structural refactoring, production safety fixes, and test coverage improvements identified by a code audit.

| # | Item | Priority | Effort |
|---|------|----------|--------|
| 8.1 | [Decompose http_request Functions](Phase%208/8.1%20—%20Decompose%20http_request%20Functions.md) | Critical | Large |
| 8.2 | [Eliminate Production Panics](Phase%208/8.2%20—%20Eliminate%20Production%20Panics.md) | High | Small |
| 8.3 | [Deduplicate Router and Asset Router Internals](Phase%208/8.3%20—%20Deduplicate%20Router%20and%20Asset%20Router%20Internals.md) | High | Medium |
| 8.4 | [Harden Build Script](Phase%208/8.4%20—%20Harden%20Build%20Script.md) | Medium | Medium |
| 8.5 | [Route Trie Optimization](Phase%208/8.5%20—%20Route%20Trie%20Optimization.md) | Medium | Medium |
| 8.6 | [Test Coverage and Edge Cases](Phase%208/8.6%20—%20Test%20Coverage%20and%20Edge%20Cases.md) | Medium | Medium |

Implementation plan: [Phase 8 Plan](Phase%208/PLAN.md)

## Ecosystem Context

Dfinity's **asset canister** (`ic-asset-certification` crate) handles static asset serving and certification. This library builds on top of that foundation, adding dynamic asset generation with file-based routing. The name `ic-asset-router` positions it as the higher-level routing layer above the existing certification infrastructure.

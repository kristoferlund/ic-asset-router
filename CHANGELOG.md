# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] — 2026-02-19

### Changed

- Decomposed `http_request` and `http_request_update` into focused helper functions.
- Extracted shared certification logic into `certify_inner` to remove duplication.
- Replaced `Vec`-based route trie children with `HashMap` for O(1) static child lookup.
- Rewrote build script scanners (`scan_pub_fns`, `scan_route_attribute`, `scan_certification_attribute`) to use `syn` AST parsing instead of fragile text matching.
- Replaced `ic_cdk::trap` in certification paths with graceful degradation.

### Fixed

- Fixed memory leak in `url_decode` caused by `.leak()` — now uses `.into_owned()`.
- Route parameters and wildcards are now URL-decoded in generated wrapper code.
- Re-certifying a path no longer leaves stale entries in the certification tree.
- Build script filesystem operations now produce contextual error messages instead of bare `.unwrap()`.

### Removed

- Removed dead code (`certify_dynamic_response`, `certify_dynamic_response_inner`).

## [0.1.0] — 2026-02-19

First release.

# ic-asset-router

Rust library for file-system-based HTTP routing with IC response certification for Internet Computer canisters. Includes a proc macro crate (`macros/`) and a build script that generates route trees from `src/routes/` directory structures.

## Build and test

```sh
cargo check              # type-check library
cargo test               # unit tests (283) + doc tests
cargo doc --no-deps      # verify rustdoc builds cleanly
```

e2e tests use PocketIC and require building a wasm test canister. Ask before running:

```sh
cd tests/e2e && cargo test
```

Example canisters are in `examples/`. Each has its own Cargo.toml with a path dependency on the library. They build with `dfx build` and require a running local replica.

## Git rules

- Do NOT commit or push unless explicitly instructed.
- Do NOT amend commits that have been pushed.
- Do NOT force push.

## Code standards

- All public functions, structs, enums, and traits must have rustdoc comments. Verify with `cargo doc --no-deps` (should produce no warnings).
- No `#[allow(missing_docs)]` on public items.
- Run `cargo check` after any code change to verify compilation before reporting completion.
- Prefer editing existing files over creating new ones. Do not create markdown or documentation files unless asked.


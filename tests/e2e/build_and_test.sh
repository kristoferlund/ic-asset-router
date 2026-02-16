#!/usr/bin/env bash
# Build the test canister WASM and run the PocketIC E2E tests.
#
# Prerequisites:
#   - Rust toolchain with wasm32-unknown-unknown target installed
#   - PocketIC server binary available (see README below)
#
# Usage:
#   cd tests/e2e && ./build_and_test.sh
#
# The script will:
#   1. Build the test canister as a release WASM
#   2. Run the E2E test crate against the built WASM
#
# PocketIC server binary:
#   The pocket-ic crate automatically downloads and caches the PocketIC server
#   binary on first use. Alternatively, set POCKET_IC_BIN to point to a
#   pre-downloaded binary:
#
#     export POCKET_IC_BIN=/path/to/pocket-ic-server
#
#   Download from: https://github.com/dfinity/pocketic/releases

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEST_CANISTER_DIR="$SCRIPT_DIR/test_canister"
E2E_DIR="$SCRIPT_DIR"

echo "=== Step 1: Building test canister WASM ==="
cargo build \
    --target wasm32-unknown-unknown \
    --release \
    --manifest-path "$TEST_CANISTER_DIR/Cargo.toml"

WASM_PATH="$TEST_CANISTER_DIR/target/wasm32-unknown-unknown/release/test_canister.wasm"
if [ ! -f "$WASM_PATH" ]; then
    echo "ERROR: WASM not found at $WASM_PATH"
    exit 1
fi
echo "WASM built: $WASM_PATH ($(wc -c < "$WASM_PATH") bytes)"

echo ""
echo "=== Step 2: Running E2E tests ==="
cargo test --manifest-path "$E2E_DIR/Cargo.toml" -- --test-threads=1 "$@"

echo ""
echo "=== All E2E tests passed ==="

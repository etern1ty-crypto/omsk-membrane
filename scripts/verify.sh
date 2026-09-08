#!/usr/bin/env bash
# Normalize formatting and run the actual Rust + black-box checks.
set -euo pipefail
cd "$(dirname "$0")/.."
for tool in cargo rustc python3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'Required tool missing: %s. Verification did NOT pass.\n' "$tool" >&2
    exit 127
  fi
done
rustc --version
cargo --version
# This step writes canonical formatting. Commit its changes after first use.
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --offline --locked --workspace --all-targets -- -D warnings
cargo test --offline --locked --workspace --all-targets
cargo test --offline --locked --workspace --doc
cargo build --offline --locked -p reactor --bin omsk
cargo build --offline --locked --release -p reactor --bin omsk
RUSTDOCFLAGS='-D warnings' cargo doc --offline --locked --workspace --no-deps
python3 scripts/preflight.py
python3 scripts/e2e.py --binary target/debug/omsk
python3 scripts/e2e.py --binary target/release/omsk

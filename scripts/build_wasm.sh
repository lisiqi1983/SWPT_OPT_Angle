#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required. Install Rust from https://rustup.rs/ first." >&2
  exit 1
fi

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "wasm-pack is missing; installing with cargo install wasm-pack ..." >&2
  cargo install wasm-pack
fi

wasm-pack build "$ROOT_DIR/crates/swpt_core" \
  --target web \
  --release \
  --out-dir "$ROOT_DIR/web/pkg"

rm -f "$ROOT_DIR/web/pkg/.gitignore"

echo "WASM package written to $ROOT_DIR/web/pkg"

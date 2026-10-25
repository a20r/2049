#!/usr/bin/env bash
# Builds the wasm module and assembles the static site in dist/.
# Needs: rustup target wasm32-unknown-unknown, wasm-bindgen-cli matching Cargo.lock.
set -euo pipefail
cd "$(dirname "$0")"

PROFILE="${1:-release}"
if [ "$PROFILE" = "release" ]; then
  cargo build --release --target wasm32-unknown-unknown -p nexus2049
  WASM=target/wasm32-unknown-unknown/release/nexus2049.wasm
else
  cargo build --target wasm32-unknown-unknown -p nexus2049
  WASM=target/wasm32-unknown-unknown/debug/nexus2049.wasm
fi

rm -rf dist
mkdir -p dist/pkg
cp -r web/. dist/
wasm-bindgen --target web --no-typescript --out-dir dist/pkg "$WASM"

# Stamp the service worker so every build installs into its own cache.
BUILD="$(git rev-parse --short HEAD 2>/dev/null || date -u +%Y%m%d%H%M%S)-$(date -u +%s)"
sed -i.bak "s/__BUILD__/$BUILD/" dist/sw.js && rm -f dist/sw.js.bak
grep -q "$BUILD" dist/sw.js
ls -la dist/pkg

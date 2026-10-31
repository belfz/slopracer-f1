#!/bin/sh
# Build the browser version into web/pkg. Serve the web/ directory with any
# static file server, e.g.: python3 -m http.server -d web 8000
set -e
cd "$(dirname "$0")/.."
rm -rf web/pkg
wasm-pack build --release --target web --out-dir web/pkg --no-typescript --no-pack
cp assets/icon.svg web/pkg/icon.svg

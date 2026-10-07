#!/usr/bin/env bash
# Compile le jeu pour le navigateur dans web/ (ouvre ensuite web/index.html via un serveur HTTP).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --profile web --target wasm32-unknown-unknown --target-dir target-wasm
wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name viking_dash \
  target-wasm/wasm32-unknown-unknown/web/viking-dash.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    -o web/pkg/viking_dash_bg.wasm web/pkg/viking_dash_bg.wasm
fi
rm -rf web/assets && cp -R assets web/assets
echo "OK : lance par exemple  python3 -m http.server 8080 -d web  puis ouvre http://localhost:8080"

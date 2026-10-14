#!/usr/bin/env bash
# Build Fanimate for the browser into <out-folder> (default dist/web): an
# index.html, the WebAssembly module and its JavaScript glue. Serve the
# folder over http(s) to use it; see docs/web.md.
#
#   scripts/build-web.sh [out-folder]
#
# Needs the wasm32-unknown-unknown target (rust-toolchain.toml installs it)
# and wasm-bindgen-cli at the same version as the wasm-bindgen crate in
# Cargo.lock; without one on PATH, this script downloads that release.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist/web}"
cd "$root"

version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
bindgen="wasm-bindgen"
if ! command -v wasm-bindgen > /dev/null || [ "$(wasm-bindgen --version | cut -d' ' -f2)" != "$version" ]; then
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) triple=x86_64-unknown-linux-musl ;;
    Darwin-arm64) triple=aarch64-apple-darwin ;;
    Darwin-x86_64) triple=x86_64-apple-darwin ;;
    MINGW*|MSYS*|CYGWIN*) triple=x86_64-pc-windows-msvc ;;
    *) echo "build-web: install wasm-bindgen-cli $version (cargo install wasm-bindgen-cli --version $version)" >&2; exit 1 ;;
  esac
  tools="$root/target/wasm-bindgen-$version"
  # check the program, not the folder: CI's build cache can restore the
  # folder without it
  if [ ! -x "$tools/wasm-bindgen" ] && [ ! -x "$tools/wasm-bindgen.exe" ]; then
    mkdir -p "$tools"
    curl -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$version/wasm-bindgen-$version-$triple.tar.gz" \
      | tar -xz -C "$tools" --strip-components=1
  fi
  bindgen="$tools/wasm-bindgen"
fi

cargo build -p fanimate-web --release --target wasm32-unknown-unknown
mkdir -p "$out"
"$bindgen" --target web --no-typescript --out-dir "$out" \
  target/wasm32-unknown-unknown/release/fanimate_web.wasm
cp crates/fanimate-web/index.html "$out/"
cp brand/app-icon/favicon.svg "$out/"
echo "built $out"

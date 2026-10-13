#!/usr/bin/env bash
# Rebuilds the Wasm module and its JavaScript bindings from this checkout.
#
# Requirements: rustup (the toolchain pinned in rust-toolchain.toml is used),
# the wasm32-unknown-unknown target, wasm-bindgen-cli 0.2.129 and Node.js.
# The committed files in wasm/pkg are the reviewed output. This script does not
# reapply the formatter or linter, so its output may differ from them in layout.
set -euo pipefail
cd "$(dirname "$0")"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

[[ "$(wasm-bindgen --version)" == 'wasm-bindgen 0.2.129' ]]
source_root="$(cd .. && pwd -P)"
cargo_root="$(cd "${CARGO_HOME:-$HOME/.cargo}" && pwd -P)"
mkdir -p target
rustc +1.99.0 --edition=2021 -D warnings -F unsafe_code reproducible-rustc.rs -o target/reproducible-rustc
wrapper_hash="$(sha256_of reproducible-rustc.rs)"
compiler_flags=(
  '-C' 'link-arg=--max-memory=134217728'
  '-C' "metadata=$wrapper_hash"
  "--remap-path-prefix=$source_root=/siediff"
  "--remap-path-prefix=$cargo_root=/cargo"
)
printf -v encoded_flags '%s\x1f' "${compiler_flags[@]}"
SIEDIFF_SOURCE_ROOT="$source_root" SIEDIFF_CARGO_ROOT="$cargo_root" \
  RUSTC_WRAPPER="$PWD/target/reproducible-rustc" \
  CARGO_ENCODED_RUSTFLAGS="${encoded_flags%$'\x1f'}" \
  cargo +1.99.0 build --locked --target wasm32-unknown-unknown --target-dir target --release
wasm-bindgen target/wasm32-unknown-unknown/release/siediff_wasm.wasm --target web --out-dir pkg
node prepare-bindings.mjs
node verify.mjs --write

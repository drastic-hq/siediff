# WebAssembly build

This directory builds a WebAssembly binding for the SIEDiff engine. The binding calls the same Rust comparison code as the command line tool. It has no separate comparison logic.

Files:

- `pkg/`: the prebuilt ES module (`siediff_wasm.js`), its TypeScript declarations and the Wasm binary. These are committed so you do not need a Rust toolchain to use the binding.
- `native.mjs`: a small loader that wraps `compare_inputs` for Node or Bun.
- `build.sh`: rebuilds `pkg/` from source.
- `verify.mjs`: checks the source and artifact hashes against `manifest.json`. `node verify.mjs --write` regenerates the manifest.
- `prepare-bindings.mjs`: post-processes the generated bindings to a synchronous, precompiled-module entry point. `build.sh` runs it.

## Rebuild

Requirements: rustup, the target `wasm32-unknown-unknown`, `wasm-bindgen-cli` 0.2.129 and Node.js.

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.99.0
cargo install wasm-bindgen-cli --version 0.2.129 --locked
./wasm/build.sh
node wasm/verify.mjs
```

`build.sh` uses the toolchain pinned in `rust-toolchain.toml`. It maps the checkout and Cargo cache paths to fixed names so that embedded paths do not reveal the builder's directories. It limits linear memory to 128 MiB. It then rewrites `wasm/manifest.json`.

When tested with Rust 1.99.0 and wasm-bindgen-cli 0.2.129, the rebuilt `siediff_wasm_bg.wasm` matched the committed binary byte for byte. The generated JavaScript and declarations differ from the committed files in formatting (quote style and line layout), because the formatter and linter used for the committed files are not part of this repository. Review the diff before committing a rebuilt `pkg/`.

## Using the module

```js
import { readFile } from "node:fs/promises";
import { initSync, compare_inputs } from "./pkg/siediff_wasm.js";

const module = new WebAssembly.Module(await readFile(new URL("./pkg/siediff_wasm_bg.wasm", import.meta.url)));
initSync({ module });

const report = JSON.parse(compare_inputs(leftBytes, rightBytes, rightIsSnapshot, renumbered));
```

In a browser, compile the module from a `fetch` response instead:

```js
const module = new WebAssembly.Module(await (await fetch("siediff_wasm_bg.wasm")).arrayBuffer());
```

`compare_inputs` takes two `Uint8Array`s, two booleans and returns a JSON report string. The left input is always SIE. `rightIsSnapshot` selects the JSON snapshot format for the right input. `renumbered` selects the renumbered comparison mode. Input is read as CP437. On refusal the call throws an error code string.

Each `initSync` call creates a new instance with fresh memory. Call it before each comparison, as `native.mjs` does, so that a trap in an earlier call cannot affect a later one.

import { readFileSync, writeFileSync, unlinkSync } from "node:fs";
import assert from "node:assert/strict";

const path = new URL("pkg/siediff_wasm.js", import.meta.url);

const generated = readFileSync(path, "utf8");

const end = generated.indexOf("async function __wbg_load(");

assert.ok(end > 0, "Pinned wasm-bindgen output changed; review the binding adapter");

// Keep the generated ABI marshalling. Only a precompiled module can initialize
// this owner, and each synchronous call gets fresh memory after a prior trap.
const binding = generated
  .slice(0, end)
  .replace("let wasmModule, wasmInstance, wasm;", "let wasm;")
  .replace(
    "function __wbg_finalize_init(instance, module)",
    "function __wbg_finalize_init(instance)",
  )
  .replace("    wasmInstance = instance;\n", "")
  .replace("    wasmModule = module;\n", "");

writeFileSync(
  path,
  `${binding}
export function initSync({ module }) {
  const instance = new WebAssembly.Instance(module, __wbg_get_imports());
  return __wbg_finalize_init(instance);
}
`,
);

writeFileSync(
  new URL("pkg/siediff_wasm.d.ts", import.meta.url),
  `export function compare_inputs(left: Uint8Array, right: Uint8Array, rightIsSnapshot: boolean, renumbered: boolean): string;
export function initSync(input: { module: WebAssembly.Module }): WebAssembly.Exports;
`,
);

unlinkSync(new URL("pkg/siediff_wasm_bg.wasm.d.ts", import.meta.url));

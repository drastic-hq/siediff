import { readFileSync } from "node:fs";
import { initSync, compare_inputs } from "./pkg/siediff_wasm.js";

const module = new WebAssembly.Module(
  readFileSync(new URL("./pkg/siediff_wasm_bg.wasm", import.meta.url)),
);

export function compareInputs(left, right, rightIsSnapshot, renumbered) {
  initSync({ module });

  return compare_inputs(left, right, rightIsSnapshot, renumbered);
}

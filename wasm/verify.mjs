import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));

const files = (directory) =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? files(join(directory, entry.name)) : [join(directory, entry.name)],
  );

const paths = [
  "Cargo.toml",
  "Cargo.lock",
  "rust-toolchain.toml",
  "wasm/Cargo.toml",
  "wasm/build.sh",
  "wasm/reproducible-rustc.rs",
  "wasm/prepare-bindings.mjs",
  ...files(join(root, "src"))
    .filter((path) => path.endsWith(".rs"))
    .map((path) => relative(root, path)),
  ...files(join(root, "wasm/src")).map((path) => relative(root, path)),
  ...files(join(root, "wasm/pkg")).map((path) => relative(root, path)),
].sort((left, right) => {
  if (left === right) return 0;

  return left < right ? -1 : 1;
});

const hashes = Object.fromEntries(
  paths.map((path) => [
    path,
    createHash("sha256")
      .update(readFileSync(join(root, path)))
      .digest("hex"),
  ]),
);

const receipt = {
  schema: "siediff.wasm-build.v1",
  rust: "1.99.0",
  wasmBindgen: "0.2.129",
  target: "wasm32-unknown-unknown",
  maximumLinearMemoryBytes: 134217728,
  hashes,
};

const manifest = join(root, "wasm/manifest.json");

if (process.argv[2] === "--write") writeFileSync(manifest, JSON.stringify(receipt, null, 2) + "\n");
else
  assert.deepEqual(
    receipt,
    JSON.parse(readFileSync(manifest, "utf8")),
    "SIEDiff runtime/source changed: rebuild and qualify the retained Wasm artifact",
  );

console.log("SIEDiff runtime source and artifact hashes verified");

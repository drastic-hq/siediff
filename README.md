# SIEDiff

SIEDiff compares two Swedish SIE accounting exports, or an SIE export and a JSON snapshot of the same book. Typical use: checking what changed when a company moves its books from one accounting system to another.

It reports three things separately:

- **What moved**: vouchers, rows and account balances that agree or differ between the two inputs.
- **What differs**: each difference, with the exact fields and source lines it came from.
- **What cannot be established**: checks the inputs cannot support, such as migration readiness or accountant acceptance. These are reported as indeterminate. SIEDiff never guesses.

It is a read-only observer. It does not judge whether the accounting is correct, does not approve a migration, and does not write to any ledger. An exit status of 0 means the selected check passed. It does not mean the books are right.

Try it in the browser: https://drastic.dev/sie-diff

## Install and build

You need Rust. The toolchain version is pinned in `rust-toolchain.toml`, and rustup installs it automatically.

```sh
cargo build --locked --release
```

The binary is `target/release/siediff`.

## Command line

```sh
siediff diff left.se right.se --json
siediff diff left.se right.se --mode renumbered --json
siediff diff source.se snapshot.json --right-format snapshot --json
siediff controls ledger.se controls.json --json
siediff validate file.se
siediff inspect file.se
siediff --help
```

- `diff` compares two inputs. `--mode renumbered` lets voucher numbers change between the two inputs and pairs vouchers only when the identity is unique. `--claim` selects the single check that decides the exit status.
- `controls` checks a ledger against a manifest of independent control balances or movements, such as bank balances. Each control reports pass, fail or indeterminate.
- Snapshot mode compares an SIE file with a JSON snapshot of the same book. The snapshot describes the book's vouchers and balances in the format given by `schemas/snapshot.schema.json`. Whatever system holds the ledger produces that file. SIEDiff only reads it.
- `validate` and `inspect` check that a file parses and show what SIEDiff read from it.

Without `--json`, SIEDiff prints a summary. `--json` prints the full report on stdout, which may contain private details from your books. `--output NEW_FILE` writes the full report to a new file with mode 0600. It refuses to overwrite an existing file.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | The selected check passes. |
| 1 | The selected check fails. |
| 2 | Invalid input. |
| 3 | Indeterminate or not applicable. |
| 4 | A resource limit was reached. |
| 5 | An I/O error. |

## WebAssembly and JavaScript

The `wasm/` directory contains a WebAssembly build of the same engine, with the prebuilt files in `wasm/pkg/`. The JavaScript API is:

```js
compare_inputs(left, right, rightIsSnapshot, renumbered) // returns a JSON report string
```

- `left` and `right` are `Uint8Array`s with the file bytes. `left` is always an SIE file.
- `rightIsSnapshot` is `true` when `right` is a JSON snapshot.
- `renumbered` selects the renumbered comparison mode.
- On refusal the call throws an error code string.
- The JavaScript API reads SIE input as CP437 only. Use the CLI for other encodings.

A minimal Node example:

```js
import { readFile } from "node:fs/promises";
import { initSync, compare_inputs } from "./wasm/pkg/siediff_wasm.js";

const module = new WebAssembly.Module(await readFile("./wasm/pkg/siediff_wasm_bg.wasm"));
initSync({ module });

const left = new Uint8Array(await readFile("left.se"));
const right = new Uint8Array(await readFile("right.se"));
const report = JSON.parse(compare_inputs(left, right, false, false));
```

To rebuild the Wasm files, see [wasm/README.md](wasm/README.md).

## Report and input formats

The JSON schemas live in `schemas/`:

| Schema | `$id` |
| --- | --- |
| Report | `urn:siediff:report:v1` |
| Controls | `urn:siediff:controls:v1` |
| Mapping | `urn:siediff:mapping:v1` |
| Policy | `urn:siediff:policy:v1` |
| Snapshot | `urn:siediff:snapshot:v1` |
| Work basis | `urn:siediff:work-basis:v1` |

The schemas describe the JSON shapes. The Rust code also checks things a schema cannot express, such as exact money arithmetic, dates, duplicate keys, voucher graph validity and resource limits.

## Limits

- SIEDiff observes and reports. It does not judge accounting correctness, tax treatment or whether a book is complete.
- Matching exports do not prove that nothing is missing. Undisclosed accounts and missing evidence cannot be found by comparing two files.
- Account totals are weaker evidence than matching postings.
- Accountant acceptance is never reported as passed. It is outside the tool.
- The test fixtures are invented. Company names, organisation numbers and amounts do not correspond to real entities.

## Tests

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
python3 -m unittest discover -s oracle -p 'test_*.py'
python3 scripts/cli_journeys.py target/release/siediff target/cli-journeys
```

The Python oracle is a small, separate reference implementation for the fixtures. It is a cross-check, not a production parser. `cli_journeys.py` runs the release binary against the fixtures and writes its results to the directory you name.

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

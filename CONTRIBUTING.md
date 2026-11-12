# Contributing to interop

Bugs and gaps go to the [issue tracker](https://github.com/Scouren/Interoperate/issues). For anything larger than a fix, open an issue
before the pull request. A change is ready when CI is:

```sh
cargo fmt --check
cargo clippy --all-targets --features cli -- -D warnings
cargo test --features cli --no-fail-fast
nix develop -c ./build.sh      # the committed interop.wasm must equal what this writes
```

Never put a commercial PDK's data in an issue, a test, a fixture or a commit: no cell, library
or parameter names and no model text. `tests/fixtures/cadence/` and `live/local/` are
local-only for that reason.

## Code

| File | Does |
|---|---|
| `src/design.rs`, `src/nets.rs`, `src/profile.rs` | Schemify files as tables, connectivity, coordinate/orientation/name rules |
| `src/expr/`, `src/core.rs` + `core/` | Schemify's expression language (scalar mode, calculator checks) and its built-in `core` device library: Interop's own copies until Schemify's public SDK ships them |
| `src/sfyx.rs` | the exchange-file codec |
| `src/virtuoso/` | `write/` (export), `read/` (import), `kinds` (the record kinds), `ael` (values), `devices` (analogLib table), `ade/` (maestro) |
| `src/merge.rs`, `src/remap.rs`, `src/map.rs` | keeping Schemify-only data, re-mastering, `interop.map` |
| `src/xschem_out.rs`, `src/translate.rs` … | xschem export and import |
| `src/job.rs` | one conversion as pure steps, driven by `src/plugin.rs` (editor) and `src/cli/` (one module per subcommand) |
| `skill/*.il` | the parts of `schemify.il` |
| `tests/` | `skill/` (the SKILL interpreter and `mockdb/`, the mock Virtuoso), `virtuoso_rust`, `virtuoso_skill`, `skill_lint`, `virtuoso_e2e`, `ade`, `exercise1` (a course's benches: maestro + OCEAN goldens), `xschem_export`, and the xschem import suites |

## Development

`nix develop` has the pinned Rust 1.95.0 (rust-overlay, with rust-lld), xschem, ngspice and wasmtime. Interop builds on
Schemify's public SDK only: `schemify-format`, `schemify-abi` and (tests) `schemify-plugin`
from [Scouren/Schemify-Plugins](https://github.com/Scouren/Schemify-Plugins), pinned to one commit in
`Cargo.toml`. Nothing else of Schemify's is linked, and the plugin never runs Schemify's
netlister or simulator. Tests read copies of the core fixtures in `tests/fixtures/schemify/`.

- **Netlists in tests.** The oracles that compared Schemify's netlist of an import with
  Virtuoso's, xschem's or the original's now check the netlist's *input*: the cell files must
  equal a snapshot in `tests/fixtures/netlisted/`, taken from the code whose netlists agreed
  (`common::frozen_input`). Re-bless (`SCHEMIFY_BLESS=1`) only after netlisting the new input
  in a Schemify development build.
- **`cadence-records`** (feature, off by default): a multi-test Maestro view and device
  parameters as bench `test` and `param` records. Schemify's public format does not have
  them yet, so the default build imports the first test and reports the rest; the tests that
  need them are ignored without the feature. To build it, patch `schemify-format` to a
  checkout that has them (`--config 'patch."https://github.com/Scouren/Schemify-Plugins".schemify-format.path="…"'`).
- **`maestro-results`** refuses for now: the run directory is named by the design hash of
  Schemify's run, which its public interface does not offer.

| Command | Does |
|---|---|
| `cargo test --features cli` | every native test; the corpus ones skip without `SCHEMIFY_CORPUS` |
| `./build.sh` | rebuilds `interop.wasm` (reproducible; commit it with the change) |
| `SCHEMIFY_REQUIRE_XSCHEM=1 cargo test --features cli --test guest --test xschem_export` | the built guest through Schemify's host, the export netlisted by real xschem |
| `corpus/fetch.sh && corpus/golden.sh && NETLIST=none corpus/import_all.sh DIR` | every corpus bench imported and checked (nightly) |
| `SPICECMP=<spicecmp> corpus/roundtrip.sh DIR` | the corpus through xschem → Schemify → xschem, re-netlisted by real xschem and compared with the goldens (needs a spicecmp build: not in Schemify's public SDK) |
| `determinism/run.sh <schemify-interop command> <out>` | the import every platform must write byte for byte |
| `cd fuzz && cargo +nightly fuzz run xschem_read` | the fuzzers (`fuzz/seeds.sh` seeds them) |

CI (`.github/workflows/`): `ci.yml` runs the suite on Linux, macOS and Windows, compares the
import across them and WASI, rebuilds the guest and fails when the committed module differs;
`nightly.yml` imports the whole corpus, runs the fuzzers, and the suite against the public SDK's `main`;
`release.yml` builds the CLI for every platform into a draft release on a `v*` tag.

### Parallel lanes

Interop's remaining Cadence work runs in parallel lanes, each owning disjoint files. Every
registration point already lists each lane's slot, so a lane adds code to its own files and
edits no shared list:

| Lane | Owns |
|---|---|
| L1 Maestro XML dump/load | `src/virtuoso/ade/import.rs`, `skill/sfy_ade.il`, `tests/skill/mockdb/mae.rs` |
| L2 label cosmetics | `src/virtuoso/read/labels.rs`, `src/virtuoso/write/labels.rs`, the label parts of `skill/sfy_cells.il` |
| L3 config views | `src/virtuoso/config.rs`, `skill/sfy_cfg.il`, `tests/skill/mockdb/hdb.rs` |
| L4 results to waves | `src/virtuoso/results.rs`, `skill/sfy_results.il`, `src/cli/maestro_results.rs` |
| L5 Monte Carlo and optimizer | `src/virtuoso/ade/runmode.rs` |
| L6 multi-test and run plans | `src/virtuoso/ade/export.rs` |
| L7 ADE L | `skill/sfy_adel.il`, `tests/skill/mockdb/asi.rs` |
| L8 layout through KLayout | `src/klayout.rs`, `skill/sfy_layout.il`, `src/cli/layout.rs` |
| L9 design-library CDF callbacks | `src/virtuoso/read/cdf.rs`, `skill/sfy_cdfcb.il` |
| L10 the PDK side | `src/pdk_models.rs`, `src/pdk_scan.rs` |
| pdk-map | `src/pdk_map.rs`, `src/cli/pdk_map.rs`, `skill/sfy_pdkmap.il`, `live/checks/pdkmap_*` |

Each of L1 to L9 also owns its live check, `live/checks/l<N>_*.il` (`live/checks/run.sh`).
The host comes from `SFY_HOST` (its ssh name; required, no default): a lane with a driver
runs `SFY_HOST=<host> live/checks/l<N>_drive.sh`, which ships `live/checks` and starts
`run.sh` there; any other check runs as `run.sh`'s header shows, `ssh "$SFY_HOST" bash
sfytest/checks/src/run.sh <check>`.
The registration points: a new `.sfyx` record kind is a row in its lane's section of
`src/virtuoso/kinds.rs`; mock Cadence functions are rows in the area's `ROWS` and bodies in
its `call` (`tests/skill/mockdb/`); the SKILL parts are already in `src/skill.rs`'s `PARTS`
(load order, `schemify.il` and the lint), and the CLI subcommands in `src/cli/mod.rs`.

Rules for every lane:

- In a lane, never commit `interop.wasm` or `Cargo.lock`: the integration rebuilds both (on
  `main` they are committed with the change, as above). A lane's `Cargo.lock` holds the
  local path sources of its `.cargo/config.toml` patches, so stage
  files by name (`git add <paths>`, never `git commit -a` or `git add -A`) and, before each
  commit, check that `git diff --cached --name-only` lists none of `Cargo.lock`,
  `interop.wasm`, `.cargo/` and `tests/fixtures/cadence/`.
- The spec text (`docs/spec/interop.md`, in the Schemify repository) and the README's "What does
  not cross" are edited only at integration.
- Build with a `CARGO_TARGET_DIR` of the lane's own.
- The Cadence probe fixtures in `tests/fixtures/cadence/` are local-only (listed in
  `.git/info/exclude`): a test that reads them gets its path from `common::cadence_fixture`
  and returns when it gets `None`, so it skips where they are absent.

To move to a newer Schemify SDK: set every `rev` in `Cargo.toml` to the new commit,
`cargo update -p schemify-abi -p schemify-format -p schemify-plugin`, `./build.sh`, test, and
commit `Cargo.toml`, `Cargo.lock` and `interop.wasm` together.

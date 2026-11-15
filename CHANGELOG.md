# Changelog

All notable changes to interop are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No version has been tagged yet, so
everything is under Unreleased, summarized by the date of the commits.

## [Unreleased]

### 2026-11-15

- Test fixtures copied from live Virtuoso sessions are hand-written now: the schematic gap
  dumps, the 256-parameter cap cell, a results point and a Monte Carlo history.
- The ADE analysis forms table (`src/virtuoso/ade/forms.tsv`) is hand-written and covers the
  common analyses: a field it does not list is kept as set, and reported when it is not a
  Spectre parameter.
- Comments name no product release, tool error code or vendor manual.

### 2026-11-14

- Store listing (`listing`) and this changelog.
- Test fixtures and comments carry no vendor documentation: the `cdfDump` reader's fixtures
  are synthetic, and the mock Virtuoso's signature tables are described as arity tables.
- `pdk-scan` and the README name no internal library: a Spectre kit's unread lines are "lines
  the Spectre reader does not read".

### 2026-11-11

- `virtuoso-pull` and `virtuoso-push`: one ssh connection each way and nothing installed on
  the Virtuoso host. `--ref` adds a library to analogLib and basic instead of replacing them.
- `pdk-remote-models`: a remote kit's models for the project's simulator, written into the
  project's `set PDK_MODELS`.
- `pdk-models`: a dialect is named as the Schemify simulator that reads it; the report names
  each converted entry (`models <absolute path>`).
- Maestro: Monte Carlo samples are job keys, `dcOpInfo` and pac families reach the raw file;
  a swept variable stored as `start:step:stop` goes back in that form; a spec with a bound is
  an optimizer goal only in the `cadence-records` format.
- `pdk-map`: each cell's spectre `propMapping` is dumped, so a parameter the CDF holds under
  another name still pairs.
- `live/host.sh`: one local entry point for the Virtuoso host (sync, build, kit, check).
  Checks that need a site's PDK stay in `live/local/`, untracked.
- README rewritten: what and why first, a quick start, requirements, a command table; the code
  map, development notes and lane rules moved to CONTRIBUTING.md.

### 2026-11-06 to 2026-11-07

- Builds on Schemify's public SDK only (`schemify-format`, `schemify-abi`,
  `schemify-plugin` from Scouren/Schemify-Plugins, pinned to `39da966`).
- Every URL points at the Scouren organization.

### 2026-11-04

- Cadence work in parallel lanes, each with a live check on a real Virtuoso:
  - Cosmetics: every object that survives a redraw keeps how Virtuoso drew it.
  - Maestro setups round-trip in full (setup XML, `active.state`, `maestro.sdb`), a view of
    several tests maps onto test records, and Monte Carlo and optimizer setups map onto
    `montecarlo` and `optimize` records.
  - Config views: `expand.cfg` rules map onto bench config records and back.
  - ADE L states map onto the bench like a Maestro test.
  - Maestro histories become one imported Schemify run (the `maestro-results` command).
  - Layout views cross as GDS through KLayout (`layout-export`, `layout-import`).
  - Design-library CDF callbacks of the plain assignment form become `computed` parameters.
  - Analyses cross as the Spectre statements ADE netlists.
- `pdk-map`: `interop.map` rows from a kit's scanned cells and a dump of its Virtuoso
  libraries, made locally or on the kit host over ssh.
- `pdk-models`: a kit's model files converted from one simulator dialect to another, losses
  reported by diagnostic code.
- xschem import from the plugin (`interop.xschem_import`) as well as the command line;
  schematic drawings, Verilog text and a file fingerprint cross; more xschem settings become
  report rows.
- Virtuoso import uses Virtuoso's own connectivity as the oracle: every net the import draws
  differently is a report row.
- `ciel`: runs ciel from `PATH`, else a private copy pinned to 3.2.0.
- The guest's size budget grew to 2304 KB as the lanes landed.

### 2026-11-02 to 2026-11-03

- `pdk-scan`: a PDK's device interface as Schemify cells, never card data; Spectre kits,
  corner sections and Verilog-A modules are read; unread lines are counted, never shown.
- Derived parameters: discovered, cached and refreshed from Virtuoso, without rule fitting.
- xschem export round trip through real xschem over the corpus.
- An export's directory argument names every cell file under it.

### 2026-10-18 to 2026-10-29

- A standalone repository: its own workspace, the Schemify crates pinned by git revision, the
  fixtures and corpus tooling copied in.
- Reproducible `build.sh` for `interop.wasm`, a Nix flake (the `schemify-interop` package and
  a dev shell), CI, nightly and release workflows, and the `schemify.project` manifest.

### 2026-10-15

- Fixes from a series of live Virtuoso runs: symbols, labels, arcs, net
  expressions, inherited nets, Maestro corners, specs, models and outputs.
- A live Virtuoso test kit (`live/`).
- `SfyCharacterize` measures what a device's CDF callbacks derive; gm/Id retargeting was
  removed (PDKs cross by translation).

### 2026-10-12

- First version, moved out of Schemify's core as a plugin: xschem import, Schemify → xschem
  export, the Virtuoso bridge (the SKILL dumper and loader, cell export and import, the
  `.sfyx` exchange file, `interop.map`), Maestro ⇄ bench mapping, the test-only SKILL
  interpreter and mock Virtuoso, fuzz targets, and the wasm guest.

//! `schemify-interop pdk-models` on made-up kits: every reached file converted, a kit's
//! definitions kept across a round trip between dialects, losses and refusals by code, and
//! root confinement.
#![cfg(feature = "cli")]

use std::path::{Path, PathBuf};

use interop::pdk_models;
use spicerack::syntax::Dialect;
use spicerack::syntax::resolve::{DefKind, Form};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pdk_scan");

fn run(root: &Path, entry: &str, from: &str, to: &str, out: &Path) -> (String, u8) {
    let args = [
        "--root",
        root.to_str().unwrap(),
        "--entry",
        entry,
        "--from",
        from,
        "--to",
        to,
        "-o",
        out.to_str().unwrap(),
    ];
    pdk_models::run(&args.map(String::from))
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pdk_models-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The subcircuits and models the kit at `root` defines, from `top.scs` in `d`: each as
/// `subckt <name>` or `model <name>`, lowercased (SPICE is case-insensitive), sorted. A
/// binned model is one, by its base name: ngspice's `nbin.1`, `nbin.2` are Spectre's
/// `model nbin … { 1: … 2: … }`.
fn definitions(root: &Path, d: Dialect) -> Vec<String> {
    let root = std::fs::canonicalize(root).unwrap();
    let kit = spicerack::import::kit(&[root.join("top.scs")], Form::Library, d, &root);
    let mut out: Vec<String> = kit
        .definitions()
        .iter()
        .filter_map(|def| match def.kind {
            DefKind::Subckt { .. } => Some(format!("subckt {}", def.name)),
            DefKind::Model { .. } => Some(format!("model {}", def.base())),
            _ => None,
        })
        .map(|s| s.to_ascii_lowercase())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A made-up Spectre kit (no real PDK data): subcircuits, models, a binned model and two
/// corner sections, one file each.
const KIT: [(&str, &str); 5] = [
    (
        "top.scs",
        "// A made-up Spectre kit.\nsimulator lang=spectre\ninclude \"devices.scs\"\n\
         include \"corners.scs\" section=tt\n",
    ),
    (
        "devices.scs",
        "subckt nfet_lvt (d g s b)\nparameters w=1u l=20n nf=1\nm0 (d g s b) nch_core w=w l=l\n\
         ends nfet_lvt\nmodel nch_core bsim4 type=n vth0=0.4123 toxe=1.17e-9\n\
         model pch_core bsim4 type=p vth0=-0.3876\nsubckt rhigh (p n)\n\
         parameters r=1k w=1u l=2u\nr0 (p n) resistor r=r\nends rhigh\n",
    ),
    (
        "corners.scs",
        "section tt\nparameters dvth=0.0123\ninclude \"tt_devices.scs\"\nendsection tt\n\
         section ff\nparameters dvth=-0.0456\ninclude \"ff_devices.scs\"\nendsection ff\n",
    ),
    (
        "tt_devices.scs",
        "subckt tt_only (a b)\nr0 (a b) resistor r=2k\nends tt_only\n\
         model nbin.1 bsim4 type=n lmin=1e-7 lmax=1e-6 vth0=0.41\n\
         model nbin.2 bsim4 type=n lmin=1e-6 lmax=1e-5 vth0=0.42\n",
    ),
    (
        "ff_devices.scs",
        "subckt ff_only (a b)\nr0 (a b) resistor r=1k\nends ff_only\n",
    ),
];

#[test]
fn a_spectre_kit_keeps_its_definitions_through_ngspice() {
    let (kit, ng, back) = (scratch("rt-kit"), scratch("rt-ng"), scratch("rt-back"));
    for (rel, text) in KIT {
        std::fs::create_dir_all(&kit).unwrap();
        std::fs::write(kit.join(rel), text).unwrap();
    }
    let want = "converted 5 file(s)\nend\n";
    assert_eq!(
        run(&kit, "top.scs", "spectre", "spice", &ng),
        (want.into(), 0)
    );
    assert_eq!(
        run(&ng, "top.scs", "spice", "spectre", &back),
        (want.into(), 0)
    );
    let defs = definitions(&kit, Dialect::Spectre);
    assert_eq!(defs.len(), 7, "{defs:?}");
    assert_eq!(definitions(&ng, Dialect::Ngspice), defs);
    assert_eq!(definitions(&back, Dialect::Spectre), defs);
    for d in [kit, ng, back] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn a_spectre_kit_reports_its_notes_and_refusals_by_code() {
    let kit = PathBuf::from(FIXTURES).join("spectre");
    let (direct, twice) = (scratch("direct"), scratch("twice"));
    // Every section is followed: ff's include is not in the kit, and is reported at its path;
    // the `simulator lang=spice` region is copied as text (W0027).
    let want = "converted 5 file(s)\ndiag corners.lib.scs W0027 1 simulator\n\
                diag ff_missing.scs R0006 1\nend\n";
    assert_eq!(
        run(&kit, "top.scs", "spectre", "spectre", &direct),
        (want.into(), 1)
    );
    // The Verilog-A source is copied as it is.
    let va = "va/varactor.va";
    assert_eq!(read(&direct, va), read(&kit, va));
    // Two entries reaching the same files convert each once.
    let args = [
        "--root",
        kit.to_str().unwrap(),
        "--entry",
        "top.scs",
        "--entry",
        "devices.scs",
        "--from",
        "spectre",
        "--to",
        "spectre",
        "-o",
        twice.to_str().unwrap(),
    ];
    assert_eq!(pdk_models::run(&args.map(String::from)), (want.into(), 1));
    for f in [
        "top.scs",
        "devices.scs",
        "corners.lib.scs",
        "tt_devices.scs",
    ] {
        assert_eq!(read(&direct, f), read(&twice, f), "{f}");
    }

    // To SPICE: the devices file alone is lossless, and its devices survive.
    let spice = scratch("spice");
    let (out, code) = run(&kit, "devices.scs", "spectre", "spice", &spice);
    assert_eq!((out.as_str(), code), ("converted 1 file(s)\nend\n", 0));
    let dev = read(&spice, "devices.scs").to_ascii_lowercase();
    for want in [
        ".subckt nfet_lvt d g s b",
        ".subckt rhigh p n",
        ".model nch_core",
    ] {
        assert!(dev.contains(want), "{want} in\n{dev}");
    }
    // ponytail: the whole kit does not leave Spectre, and says why by code (never a silent
    // drop): its `ahdl_include` needs OSDI on ngspice (W0004), and a `simulator lang=spice`
    // region is Spectre's text (W0001). Flip when SpiceRack handles both.
    let _ = std::fs::remove_dir_all(&spice);
    let (out, code) = run(&kit, "top.scs", "spectre", "spice", &spice);
    let want = "not written\ndiag top.scs W0004 1 ahdl_include\ndiag corners.lib.scs W0001 1 simulator\n\
                diag ff_missing.scs R0006 1\nend\n";
    assert_eq!((out.as_str(), code), (want, 2));
    assert!(!spice.exists(), "nothing written on a refusal");
    for d in [direct, twice] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn an_include_leaving_the_root_writes_nothing() {
    // `.lib escape` in top.lib includes ../../outside.inc; every section is followed.
    let kit = PathBuf::from(FIXTURES).join("kit");
    let out_dir = scratch("escape");
    let (out, code) = run(&kit, "models/top.lib", "hspice", "hspice", &out_dir);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.contains("outside.inc: outside the root, refused\n"),
        "{out}"
    );
    assert!(out.contains(" R0007 1\n"), "{out}");
    assert!(!out_dir.exists());
}

#[test]
fn bad_arguments_are_refused() {
    let kit = PathBuf::from(FIXTURES).join("spectre");
    let out_dir = scratch("bad");
    assert_eq!(run(&kit, "top.scs", "spectre", "eldo", &out_dir).1, 2);
    assert_eq!(run(&kit, "top.scs", "spectre", "cdl", &out_dir).1, 2);
    assert_eq!(run(&kit, "top.scs", "rack", "spectre", &out_dir).1, 2);
    assert_eq!(pdk_models::run(&["--root".into()]).1, 2);
    assert!(!out_dir.exists());
}

/// The made-up kit written out, and `pdk-models --host kithost … --from spectre --to <to>`
/// on it through a fake `ssh` that runs the remote command here, the host's
/// `schemify-interop` (`SFY_INTEROP`) being this build's: stdout and the exit status.
#[cfg(unix)] // the fake is an sh script
fn on_host(name: &str, to: &str) -> (PathBuf, PathBuf, String, Option<i32>) {
    use std::os::unix::fs::PermissionsExt;
    let (kit, out, bin) = (
        scratch(&format!("{name}-kit")),
        scratch(&format!("{name}-out")),
        scratch(&format!("{name}-bin")),
    );
    std::fs::create_dir_all(&kit).unwrap();
    for (rel, text) in KIT {
        std::fs::write(kit.join(rel), text).unwrap();
    }
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("ssh"), "#!/bin/sh\nshift\nexec \"$@\"\n").unwrap();
    std::fs::set_permissions(bin.join("ssh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let cli = env!("CARGO_BIN_EXE_schemify-interop");
    let o = std::process::Command::new(cli)
        .args(["pdk-models", "--host", "kithost", "--root"])
        .arg(&kit)
        .args(["--entry", "top.scs", "--from", "spectre", "--to", to, "-o"])
        .arg(&out)
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("SFY_INTEROP", cli)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&bin);
    (
        kit,
        out,
        String::from_utf8(o.stdout).unwrap(),
        o.status.code(),
    )
}

#[test]
#[cfg(unix)]
fn a_remote_kit_is_converted_on_its_host_for_the_selected_simulator() {
    // A Schemify simulator's name is the dialect: the tree is written there, and `models`
    // names the file `set PDK_MODELS` takes.
    let (kit, out, text, code) = on_host("host", "ngspice");
    let want = format!(
        "converted 5 file(s)\nmodels {}/top.scs\nend\n",
        out.display()
    );
    assert_eq!((text.as_str(), code), (want.as_str(), Some(0)));
    assert!(read(&out, "devices.scs").contains(".subckt nfet_lvt"));
    for d in [kit, out] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
#[cfg(unix)]
fn a_remote_kit_in_the_simulators_dialect_is_used_where_it_is() {
    let (kit, out, text, code) = on_host("same", "spectre");
    let want = format!(
        "converted 0 file(s)\nmodels {}/top.scs\nend\n",
        kit.display()
    );
    assert_eq!((text.as_str(), code), (want.as_str(), Some(0)));
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(kit);
}

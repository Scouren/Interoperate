//! `virtuoso-pull` and `virtuoso-push` with no host: a fake `ssh` runs the remote script
//! here, and a fake `virtuoso` hands its batch file to the test, which runs it in the mock
//! Virtuoso (`tests/skill/`). So the SKILL each command sends is the SKILL that runs, on the
//! fixture libraries of `tests/fixtures/skill/build.il`.
#![cfg(all(feature = "cli", unix))] // the fakes are sh scripts

mod common;
mod skill;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::read_tree;
use skill::Interp;
use skill::mockdb::MockDb;

const BUILD: &str = include_str!("fixtures/skill/build.il");

/// The fixture libraries, and `scratch`, empty.
fn mock() -> Interp<MockDb> {
    let mut i = skill::virtuoso(&["analogLib", "basic", "pdk", "cb", "work", "scratch"]);
    i.load(BUILD).expect("build.il loads");
    i.load("(SfyFixAll)").expect("fixtures build");
    i
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sfy-remote-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("tmp")).unwrap();
    d
}

/// The fakes in `dir/bin`: `ssh` runs its command here; `virtuoso` checks its arguments,
/// names its batch file in `dir/req` and waits for `dir/done`.
fn fakes(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let d = dir.display();
    let virtuoso = format!(
        "#!/bin/sh\n[ \"$1\" = -nograph ] && [ \"$2\" = -restore ] && [ \"$4\" = -log ] || exit 9\n\
         printf '%s\\n' \"$3\" >\"{d}/req.tmp\" && mv \"{d}/req.tmp\" \"{d}/req\"\n\
         while [ ! -e \"{d}/done\" ]; do sleep 0.01; done\nrm -f \"{d}/done\"\n"
    );
    for (name, body) in [
        ("ssh", "#!/bin/sh\nshift\nexec \"$@\"\n"),
        ("virtuoso", &virtuoso),
    ] {
        let p = bin.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

/// One batch session in the mock: the session directory's files in, `run.il` run as
/// `virtuoso -restore` would, the files it wrote back out.
fn session(i: &mut Interp<MockDb>, run: &Path) {
    let dir = run.parent().unwrap();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        i.files.insert(
            p.display().to_string(),
            std::fs::read_to_string(&p).unwrap(),
        );
    }
    i.env
        .insert("SFY_RUN_DIR".into(), dir.display().to_string());
    let src = std::fs::read_to_string(run).unwrap();
    i.load(&src)
        .unwrap_or_else(|e| panic!("run.il: {e}\n{}", i.log));
    let prefix = format!("{}/", dir.display());
    for (p, t) in i.files.iter().filter(|(p, _)| p.starts_with(&prefix)) {
        std::fs::write(p, t).unwrap();
    }
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// `schemify-interop args` in `dir` with the fakes first on PATH and no Cadence settings;
/// its batch session runs in `i`.
fn cli(i: &mut Interp<MockDb>, dir: &Path, args: &[&str]) -> Out {
    let bin = fakes(dir);
    let (o, e) = (dir.join("stdout"), dir.join("stderr"));
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_schemify-interop"));
    c.args(args)
        .current_dir(dir)
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("TMPDIR", dir.join("tmp"))
        .stdout(std::fs::File::create(&o).unwrap())
        .stderr(std::fs::File::create(&e).unwrap());
    for (k, _) in interop::cli::ENV {
        c.env_remove(k);
    }
    c.env("VIRTUOSO", bin.join("virtuoso"));
    let mut child = c.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let status = loop {
        if let Ok(run) = std::fs::read_to_string(dir.join("req")) {
            std::fs::remove_file(dir.join("req")).unwrap();
            session(i, Path::new(run.trim()));
            std::fs::write(dir.join("done"), "").unwrap();
        }
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("{args:?} did not finish");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let read = |p: &Path| std::fs::read_to_string(p).unwrap();
    Out {
        code: status.code().unwrap(),
        stdout: read(&o),
        stderr: read(&e),
    }
}

fn ok(out: &Out, what: &str) {
    assert_eq!(
        out.code, 0,
        "{what}\nstdout:\n{}\nstderr:\n{}",
        out.stdout, out.stderr
    );
}

/// A dump of `lib/cell` made in the mock directly.
fn dump(i: &mut Interp<MockDb>, lib: &str, cell: &str) -> String {
    let ok = i
        .load(&format!(
            r#"(SfyExport "/c/d.sfyx" "{lib}" (list "{cell}"))"#
        ))
        .unwrap();
    assert!(ok.truthy(), "{}", i.files["/c/d.sfyx.log"]);
    i.files["/c/d.sfyx"].clone()
}

#[test]
fn a_pull_writes_what_the_dump_and_virtuoso_import_write() {
    let mut i = mock();
    let dir = tmp("pull");
    let out = cli(
        &mut i,
        &dir,
        &["virtuoso-pull", "p", "--host", "pdkhost", "--vlib", "work"],
    );
    ok(&out, "pull");
    assert!(out.stderr.contains("virtuoso-pull: "), "{}", out.stderr);
    assert!(out.stderr.contains(" s on pdkhost\n"), "{}", out.stderr);
    // The two steps by hand: SfyExport in Virtuoso, then virtuoso-import on its dump.
    let ok2 = i
        .load(r#"(SfyExport "/two/work.sfyx" "work" nil)"#)
        .unwrap();
    assert!(ok2.truthy(), "{}", i.files["/two/work.sfyx.log"]);
    let q = dir.join("q");
    std::fs::create_dir_all(q.join(".schemify/virtuoso")).unwrap();
    std::fs::write(
        q.join(".schemify/virtuoso/work.sfyx"),
        &i.files["/two/work.sfyx"],
    )
    .unwrap();
    let args = [
        "virtuoso-import",
        q.to_str().unwrap(),
        ".schemify/virtuoso/work.sfyx",
    ];
    assert_eq!(interop::cli::run(&args.map(String::from)), 0);
    let (p, q) = (read_tree(&dir.join("p")), read_tree(&q));
    assert_eq!(p.keys().collect::<Vec<_>>(), q.keys().collect::<Vec<_>>());
    for (k, v) in &p {
        assert!(*v == q[k], "{k} differs from the two-step import");
    }
    assert!(p.contains_key("work/inv.shcomp"), "{:?}", p.keys());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_push_into_a_scratch_library_dumps_back_as_the_pull_came() {
    let mut i = mock();
    let dir = tmp("trip");
    let pull = ["virtuoso-pull", "p", "--host", "pdkhost", "--vlib", "work"];
    ok(&cli(&mut i, &dir, &pull), "pull");
    let push = [
        "virtuoso-push",
        "p",
        "work",
        "--host",
        "pdkhost",
        "--into",
        "scratch",
    ];
    let out = cli(&mut i, &dir, &push);
    ok(&out, "push");
    assert!(out.stdout.ends_with("PASS\n"), "{}", out.stdout);
    assert!(out.stderr.contains(" s on pdkhost\n"), "{}", out.stderr);
    // The export's .sfyx and report stay in the project's scratch area.
    assert!(dir.join("p/.schemify/virtuoso/push.sfyx").is_file());
    let back = [
        "virtuoso-pull",
        "p2",
        "--host",
        "pdkhost",
        "--vlib",
        "scratch",
        "--lib",
        "work=scratch",
    ];
    ok(&cli(&mut i, &dir, &back), "pull of scratch");
    // work's symbol-only cells came in as mirrors of Virtuoso cells (`provenance virtuoso`),
    // which an export references and never writes: the push sends the others.
    let sent = ["bus", "inv", "netx"];
    let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap();
    let a = compared(&read("p/.schemify/virtuoso/work.sfyx"), "work", &sent);
    let b = compared(
        &read("p2/.schemify/virtuoso/scratch.sfyx"),
        "scratch",
        &sent,
    );
    if let Some((x, y)) = a.iter().zip(&b).find(|(x, y)| x != y) {
        panic!("the dumps differ:\n- {x}\n+ {y}");
    }
    assert_eq!((a.len(), a.last()), (b.len(), b.last()));
    assert!(a.len() > 100, "{a:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A dump's records as `virtuoso_e2e` compares a round trip's (`without_schemify_props`),
/// library `lib` read as `L`, the design cells other than `cells` left out, and label
/// parents set aside, as its export check does: Virtuoso splits a redrawn wire at a
/// junction, and a wire label may glue to either piece.
fn compared(text: &str, lib: &str, cells: &[&str]) -> Vec<String> {
    let recs = interop::sfyx::read(&common::without_schemify_props(text)).unwrap();
    common::located(&recs)
        .into_iter()
        .filter(|(at, r)| r.tag == "end" || !at.design || cells.contains(&at.cell.as_str()))
        .map(|(_, r)| {
            let mut r = r.clone();
            if r.tag == "label" {
                r.fields[1] = interop::sfyx::Value::Int(0);
            }
            common::line_of(&r).replace(&format!("\"{lib}\""), "\"L\"")
        })
        .collect()
}

/// work/amp: a pdk/nch transistor and an analogLib resistor.
const AMP: &str = r#"
(let ((cv (SfyFixOpen "work" "amp" "schematic" "schematic")))
  (schCreateInst cv (dbOpenCellViewByType "pdk" "nch" "symbol" nil "r") "M1" (SfyFixP 0 0) "R0")
  (schCreateInst cv (dbOpenCellViewByType "analogLib" "res" "symbol" nil "r") "R1" (SfyFixP 200 0) "R0")
  (schCheck cv)
  (SfyFixSave cv))
"#;

#[test]
fn only_the_ref_libraries_masters_leave_the_host() {
    let mut i = mock();
    i.load(AMP).unwrap();
    let dir = tmp("ref");
    let pull = |p: &'static str| {
        [
            "virtuoso-pull",
            p,
            "--host",
            "pdkhost",
            "--vlib",
            "work",
            "--cell",
            "amp",
        ]
    };
    // Default: analogLib and basic only. pdk/nch stays on the host; M1 keeps it as master.
    let out = cli(&mut i, &dir, &pull("p"));
    ok(&out, "pull");
    let dump = std::fs::read_to_string(dir.join("p/.schemify/virtuoso/work.sfyx")).unwrap();
    assert!(!dump.contains(r#"("lib" "pdk")"#), "{dump}");
    assert!(dump.contains(r#"("cell" "res" "ref")"#), "{dump}");
    let report = std::fs::read_to_string(dir.join("p/.schemify/virtuoso/work.report.txt")).unwrap();
    let rows: Vec<&str> = report
        .lines()
        .filter(|l| l.starts_with("needs_review:master"))
        .collect();
    assert_eq!(
        rows,
        [
            "needs_review:master\tpdk/nch\tnot in the dump and no cell in the project: 1 instance(s) keep it as their master (e.g. work/amp: M1 → pdk:nch)"
        ]
    );
    assert!(
        out.stdout
            .contains("1 master(s) not dumped and not in this project or interop.map (pdk 1)")
            && out
                .stdout
                .contains("`--ref analogLib --ref basic --ref pdk` dumps them"),
        "{}",
        out.stdout
    );
    // amp has no symbol: a bench.
    let amp = std::fs::read_to_string(dir.join("p/work/amp.shbench")).unwrap();
    assert!(amp.contains(" pdk:nch\n"), "{amp}");
    // With --ref pdk the master travels, with its record.
    let mut more = pull("p2").to_vec();
    more.extend(["--ref", "analogLib", "--ref", "basic", "--ref", "pdk"]);
    let out = cli(&mut i, &dir, &more);
    ok(&out, "pull --ref pdk");
    let dump = std::fs::read_to_string(dir.join("p2/.schemify/virtuoso/work.sfyx")).unwrap();
    assert!(
        dump.contains("(\"lib\" \"pdk\")\n(\"cell\" \"nch\" \"ref\")"),
        "{dump}"
    );
    assert!(!out.stdout.contains("not dumped"), "{}", out.stdout);
    let report =
        std::fs::read_to_string(dir.join("p2/.schemify/virtuoso/work.report.txt")).unwrap();
    assert!(!report.contains("needs_review:master"), "{report}");
    // Not dumped but in interop.map: M1 goes to the mapped cell as usual.
    let p3 = dir.join("p3");
    std::fs::create_dir_all(p3.join("fkit")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdk_map/scan/nfoo.shprim"),
        p3.join("fkit/nfoo.shprim"),
    )
    .unwrap();
    std::fs::write(
        p3.join("schemify.project"),
        "schemify 1 project\n\nlib fkit path fkit\n",
    )
    .unwrap();
    std::fs::write(
        p3.join("interop.map"),
        "map fkit:nfoo virtuoso pdk nch\n  pin d D\n  pin g G\n  pin s S\n  pin b B\n",
    )
    .unwrap();
    let out = cli(&mut i, &dir, &pull("p3"));
    ok(&out, "pull with a map");
    assert!(!out.stdout.contains("not dumped"), "{}", out.stdout);
    let amp = std::fs::read_to_string(p3.join("work/amp.shbench")).unwrap();
    assert!(amp.contains(" fkit:nfoo\n"), "{amp}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_push_without_overwrite_leaves_an_existing_cell_alone() {
    let mut i = mock();
    let dir = tmp("keep");
    let pull = [
        "virtuoso-pull",
        "p",
        "--host",
        "pdkhost",
        "--vlib",
        "work",
        "--cell",
        "inv",
    ];
    ok(&cli(&mut i, &dir, &pull), "pull");
    let push = [
        "virtuoso-push",
        "p",
        "work/inv.shcomp",
        "--host",
        "pdkhost",
        "--into",
        "scratch",
    ];
    ok(&cli(&mut i, &dir, &push), "first push");
    // An edit in Schemify, pushed without --overwrite: refused, the cell as it was.
    let f = dir.join("p/work/inv.shcomp");
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("  w 2u\n"), "{text}");
    std::fs::write(&f, text.replace("  w 2u\n", "  w 3u\n")).unwrap();
    let out = cli(&mut i, &dir, &push);
    assert_eq!(out.code, 1, "{}\n{}", out.stdout, out.stderr);
    for want in [
        "schemify-interop: virtuoso-push: refused line ",
        ": scratch/inv/schematic exists; pass ?overwrite t\n",
        "schemify-interop: virtuoso-push: 1 cellview(s) or CDF(s) already in Virtuoso were left alone; --overwrite replaces them\n",
    ] {
        assert!(out.stderr.contains(want), "{want}: {}", out.stderr);
    }
    let kept = dump(&mut i, "scratch", "inv");
    assert!(kept.contains(r#""w" "string" "2u")"#), "{kept}");
    // With --overwrite it lands.
    let mut over = push.to_vec();
    over.push("--overwrite");
    ok(&cli(&mut i, &dir, &over), "push --overwrite");
    let now = dump(&mut i, "scratch", "inv");
    assert!(now.contains(r#""w" "string" "3u")"#), "{now}");
    let _ = std::fs::remove_dir_all(&dir);
}

//! `virtuoso-pull`, `virtuoso-push`, and the batch Virtuoso session on an ssh host they and
//! `pdk-map --host` run: one `ssh <host> sh -s` per command, nothing installed there.

use super::*;
use crate::sfyx;

const USAGE: &str = "usage: schemify-interop virtuoso-pull <project-dir> --host <ssh name> --vlib <lib> [--cell <cell>]... [--ref <vlib>]... [--no-maestro] [--analoglib] [--lib L=V]...
       schemify-interop virtuoso-push <project-dir> <cell-file|dir>... --host <ssh name> [--into <vlib>] [--overwrite] [--callbacks] [--core-cells] [--lib L=V]...";

/// The settings the batch session takes from the environment the command runs in:
/// `(variable, the name the script holds it under)`. The licence variables are held aside
/// so that the setup script cannot replace them.
pub const ENV: [(&str, &str); 6] = [
    ("VIRTUOSO", "VIRTUOSO"),
    ("SFY_CADENCE_ENV", "SFY_CADENCE_ENV"),
    ("SFY_CDS_DIR", "SFY_CDS_DIR"),
    ("SFY_VIRTUOSO_TIMEOUT", "SFY_VIRTUOSO_TIMEOUT"),
    ("CDS_LIC_FILE", "SFY_CDS_LIC_FILE"),
    ("CDS_LIC_ONLY", "SFY_CDS_LIC_ONLY"),
];

/// The session's one file, which its entry point writes (a dump) or loads, as SKILL.
pub(super) const RUN_SFYX: &str = "(strcat (getShellEnvVar \"SFY_RUN_DIR\") \"/run.sfyx\")";

/// Where pull and push keep the `.sfyx` and its report in the project.
const SCRATCH: &str = ".schemify/virtuoso";

/// ` (list "a" "b")`: a SKILL list of strings (`(list)` is nil).
pub(super) fn skill_list(items: &[&str]) -> String {
    let mut s = String::from(" (list");
    for i in items {
        s.push(' ');
        sfyx::write_str(i, &mut s);
    }
    s.push(')');
    s
}

/// The script `ssh <host> sh -s` runs for subcommand `cmd`: each [`ENV`] variable `env`
/// gives (`VIRTUOSO` defaults to `virtuoso`), `schemify.il`, `run.il` (loads it, then runs
/// `call`, one entry point on [`RUN_SFYX`], in an `errset`), with `load` the text of that
/// file, then `virtuoso_batch.sh`. Its stdout is the dump `call` wrote, or with `load` the
/// log.
///
/// # Panics
///
/// When a file holds the heredoc's end line, which none can: SKILL source does not, and
/// each `.sfyx` line is one record (its strings escape newlines).
pub(super) fn batch_script(
    cmd: &str,
    call: &str,
    load: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
) -> String {
    const END: &str = "SFY_FILE_END";
    let sh = |v: &str| format!("'{}'", v.replace('\'', "'\\''"));
    let mut out = format!("# schemify-interop {cmd}\nsfy_cmd={cmd}\n");
    if load.is_some() {
        out.push_str("sfy_load=1\n");
    }
    for (var, held) in ENV {
        let v = env(var)
            .filter(|v| !v.is_empty())
            .or_else(|| (var == "VIRTUOSO").then(|| "virtuoso".into()));
        if let Some(v) = v {
            out.push_str(&format!("{held}={}\nexport {held}\n", sh(&v)));
        }
    }
    let run = format!(
        "(load (strcat (getShellEnvVar \"SFY_RUN_DIR\") \"/schemify.il\"))\n(errset {call} t)\n(exit)\n"
    );
    let il = crate::skill::schemify_il();
    out.push_str("sfy_files() {\n");
    for (f, text) in [
        ("schemify.il", Some(il.as_str())),
        ("run.il", Some(&run)),
        ("run.sfyx", load),
    ] {
        let Some(text) = text else { continue };
        assert!(text.lines().all(|l| l != END), "{f} holds the heredoc end");
        let nl = if text.ends_with('\n') { "" } else { "\n" };
        out.push_str(&format!("cat >\"$sfy_t/{f}\" <<'{END}'\n{text}{nl}{END}\n"));
    }
    out.push_str("}\n");
    out.push_str(include_str!("virtuoso_batch.sh"));
    out
}

/// Runs `script` through `ssh <host> sh -s` for `cmd`: (0, the dump or an import's log) when
/// its entry point passed; (1, the log) when it refused something, the log's `refused` lines
/// printed; (2, empty) when ssh or Virtuoso did not run, why printed (Virtuoso's setup by the
/// script). Prints the session's wall time on stderr.
pub(super) fn remote(cmd: &str, host: &str, script: &str) -> (u8, String) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // A name ssh would read as an option (`-oProxyCommand=…`) is no host.
    if host.starts_with('-') {
        eprintln!("schemify-interop: {cmd}: no ssh host name: {host}");
        return (2, String::new());
    }
    let start = std::time::Instant::now();
    let child = Command::new("ssh")
        .args([host, "sh", "-s"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            eprintln!("schemify-interop: {cmd}: ssh: {e}");
            return (2, String::new());
        }
    };
    let mut stdin = child.stdin.take().expect("piped stdin");
    let script = script.to_owned();
    let feed = std::thread::spawn(move || stdin.write_all(script.as_bytes()));
    let out = child.wait_with_output();
    let _ = feed.join();
    eprintln!(
        "schemify-interop: {cmd}: {:.1} s on {host}",
        start.elapsed().as_secs_f64()
    );
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            eprintln!("schemify-interop: {cmd}: ssh: {e}");
            return (2, String::new());
        }
    };
    let Ok(text) = String::from_utf8(out.stdout) else {
        eprintln!("schemify-interop: {cmd}: the output from {host} is not UTF-8");
        return (2, String::new());
    };
    match out.status.code() {
        Some(0) => (0, text),
        Some(1) => {
            for l in text.lines().filter(|l| l.starts_with("refused")) {
                eprintln!("schemify-interop: {cmd}: {l}");
            }
            (1, text)
        }
        _ => {
            eprintln!(
                "schemify-interop: {cmd}: the batch Virtuoso session on {host} failed ({})",
                out.status
            );
            (2, String::new())
        }
    }
}

/// `virtuoso-pull <project-dir> --host <h> --vlib <lib> ...`: `SfyExport` of `<lib>` (the
/// `--cell`s, or every cell) on host `<h>`, then `virtuoso-import` of that dump, kept at
/// `.schemify/virtuoso/<lib>.sfyx` beside its report. Only masters in `analogLib`, `basic`
/// and the `--ref` libraries are dumped; each other master that resolves to no cell
/// here is a `needs_review:master` row, counted per library in the summary. 0 imported, 1
/// refused (on the host or by the import), 2 usage, I/O or a failed session.
pub(super) fn virtuoso_pull(args: &[String]) -> u8 {
    let usage = || {
        eprintln!("{USAGE}");
        2
    };
    let Some((project, rest)) = args.split_first() else {
        return usage();
    };
    // The libraries whose masters may leave the host: Cadence's own, and each `--ref`.
    let (mut host, mut vlib, mut cells, mut refs, mut maestro) =
        (None, None, Vec::new(), vec!["analogLib", "basic"], true);
    // virtuoso-import's arguments: the dump's path goes in at 1 once the library is known.
    let mut import = vec![project.clone(), String::new()];
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--no-maestro" => maestro = false,
            k @ ("--host" | "--vlib" | "--cell" | "--ref") => {
                let Some(v) = it.next() else { return usage() };
                match k {
                    "--host" => host = Some(v.as_str()),
                    "--vlib" => vlib = Some(v.as_str()),
                    "--cell" => cells.push(v.as_str()),
                    _ if refs.contains(&v.as_str()) => {}
                    _ => refs.push(v.as_str()),
                }
            }
            _ => import.push(a.clone()),
        }
    }
    let (Some(host), Some(vlib)) = (host, vlib) else {
        return usage();
    };
    import[1] = format!("{SCRATCH}/{vlib}.sfyx");
    let Some((root, job)) = job_command("virtuoso-import", &import) else {
        return usage();
    };
    let mut call = format!("(SfyExport {RUN_SFYX} ");
    sfyx::write_str(vlib, &mut call);
    call.push_str(&skill_list(&cells));
    call.push_str(" ?ref");
    call.push_str(&skill_list(&refs));
    call.push_str(if maestro { ")" } else { " ?maestro nil)" });
    let env = |k: &str| std::env::var(k).ok();
    let script = batch_script("virtuoso-pull", &call, None, env);
    let (code, dump) = remote("virtuoso-pull", host, &script);
    if code != 0 {
        return code;
    }
    let path = root.join(&import[1]);
    if let Some(d) = path.parent()
        && let Err(e) = std::fs::create_dir_all(d)
    {
        return io_err(d, e);
    }
    if let Err(e) = std::fs::write(&path, dump) {
        return io_err(&path, e);
    }
    let writes = match drive(&root, &mut crate::job::Job::new(job)) {
        Ok(w) => w,
        Err(code) => return code,
    };
    let report = format!("{SCRATCH}/{vlib}.report.txt");
    let rows = writes
        .iter()
        .find(|(p, _)| *p == report)
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    let mut missing: std::collections::BTreeMap<&str, usize> = Default::default();
    for l in rows.lines() {
        if let Some((lib, _)) = l
            .strip_prefix("needs_review:master\t")
            .and_then(|r| r.split_once('/'))
        {
            *missing.entry(lib).or_default() += 1;
        }
    }
    if !missing.is_empty() {
        let libs: Vec<String> = missing.iter().map(|(l, n)| format!("{l} {n}")).collect();
        let flags: Vec<String> = missing.keys().map(|l| format!("--ref {l}")).collect();
        println!(
            "virtuoso-pull: {} master(s) not dumped and not in this project or interop.map ({}): their instances keep them as masters ({report}); `{}` dumps them",
            missing.values().sum::<usize>(),
            libs.join(", "),
            flags.join(" ")
        );
    }
    0
}

/// `virtuoso-push <project-dir> <cell-file|dir>... --host <h> ...`: `virtuoso-export` of the
/// cells to `.schemify/virtuoso/push.sfyx` (its report beside it), then `SfyImport` of that
/// file on host `<h>` (`?lib` from `--into`, `?overwrite`, `?callbacks`); prints the
/// import's log. Without `--overwrite` an existing cellview or CDF is left alone: a refusal,
/// counted in a hint. 0 PASS, 1 refused (export or import), 2 usage, I/O or a failed session.
pub(super) fn virtuoso_push(args: &[String]) -> u8 {
    let usage = || {
        eprintln!("{USAGE}");
        2
    };
    let Some((project, rest)) = args.split_first() else {
        return usage();
    };
    let (mut host, mut into, mut overwrite, mut callbacks) = (None, None, false, false);
    let mut export = vec![project.clone()];
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--overwrite" => overwrite = true,
            "--callbacks" => callbacks = true,
            k @ ("--host" | "--into") => {
                let Some(v) = it.next() else { return usage() };
                if k == "--host" {
                    host = Some(v.as_str());
                } else {
                    into = Some(v.as_str());
                }
            }
            "-o" => return usage(),
            _ => export.push(a.clone()),
        }
    }
    let Some(host) = host else { return usage() };
    let out = format!("{SCRATCH}/push.sfyx");
    export.extend(["-o".to_owned(), out.clone()]);
    let Some((root, job)) = job_command("virtuoso-export", &export) else {
        return usage();
    };
    let writes = match drive(&root, &mut crate::job::Job::new(job)) {
        Ok(w) => w,
        Err(code) => return code,
    };
    // ponytail: the OCEAN scripts the export writes beside the .sfyx (`ocean/`) stay here, not
    // sent; send them with the .sfyx when someone needs headless runs from a push.
    let text = writes
        .iter()
        .find(|(p, _)| *p == out)
        .and_then(|(_, b)| std::str::from_utf8(b).ok())
        .expect("the export writes its .sfyx");
    let mut call = format!("(SfyImport {RUN_SFYX}");
    if let Some(l) = into {
        call.push_str(" ?lib ");
        sfyx::write_str(l, &mut call);
    }
    let t = |b: bool| if b { "t" } else { "nil" };
    call.push_str(&format!(
        " ?overwrite {} ?callbacks {})",
        t(overwrite),
        t(callbacks)
    ));
    let env = |k: &str| std::env::var(k).ok();
    let script = batch_script("virtuoso-push", &call, Some(text), env);
    let (code, log) = remote("virtuoso-push", host, &script);
    if code == 0 {
        print!("{log}");
    }
    let kept = log
        .lines()
        .filter(|l| l.starts_with("refused") && l.ends_with("pass ?overwrite t"))
        .count();
    if kept > 0 {
        eprintln!(
            "schemify-interop: virtuoso-push: {kept} cellview(s) or CDF(s) already in Virtuoso were left alone; --overwrite replaces them"
        );
    }
    code
}

//! `pdk-models`: SpiceRack's model conversion (`crate::pdk_models`), its report printed.
//!
//! `--host <ssh name>` converts a kit where it lives, for one that must not leave its host:
//! the `schemify-interop` there runs the same conversion (one `ssh <host> sh -s`, whatever
//! the login shell), so `--root` and `-o` are directories on the host and only the report
//! comes back: its `models <path>` lines name the converted entries on the host, what `set
//! PDK_MODELS` takes for the simulator `--to` names. A kit already in that dialect is not
//! copied: the `models` lines name the kit's own files and no ssh runs.
//! `SFY_INTEROP` is the command on the host (default `schemify-interop`, from the login
//! `PATH` or `~/.local/bin`).

//!
//! `pdk-remote-models <project-dir> --host <[user@]host>` does that for a project whose kit
//! `pdks.remote` names (Schemify's pdk spec §11) and writes the answer where Schemify reads
//! it: the project's `set PDK_MODELS`. The simulator is `--to`, else the `simulator` row of
//! the project's tech map; the kit, its entry and its dialect are the record's. `--host`
//! must be the record's host: a project file alone never picks where ssh goes.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const REMOTE_USAGE: &str = "usage: schemify-interop pdk-remote-models <project-dir> --host <[user@]host> [--family <f>] [--to <simulator>] [-o <host dir>]";

/// Runs `pdk-models` on `args`; its exit status.
pub(super) fn pdk_models(args: &[String]) -> u8 {
    let (out, code) = match args.iter().position(|a| a == "--host") {
        Some(i) if i + 1 < args.len() => {
            let mut rest = args.to_vec();
            let host = rest.remove(i + 1);
            rest.remove(i);
            on_host(&host, &rest)
        }
        _ => crate::pdk_models::run(args),
    };
    print!("{out}");
    code
}

/// `pdk-models <args>` run on `host`: its report and its status (2 too when ssh or the
/// command there fails).
fn on_host(host: &str, args: &[String]) -> (String, u8) {
    let r = match crate::pdk_models::request(args) {
        Ok(r) => r,
        Err(e) => return e,
    };
    // A name ssh would read as an option (`-oProxyCommand=…`) is no host.
    if host.starts_with('-') {
        return (format!("no ssh host name: {host}\n"), 2);
    }
    if r.from == r.to {
        let root = r.root.trim_end_matches('/');
        let own: String = r
            .entries
            .iter()
            .map(|e| format!("models {root}/{e}\n"))
            .collect();
        return (format!("converted 0 file(s)\n{own}end\n"), 0);
    }
    let cmd = std::env::var("SFY_INTEROP")
        .ok()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "schemify-interop".into());
    // The command is unquoted, as VIRTUOSO is in pdk-map's script: `$HOME/bin/…` expands there.
    let mut script = format!("PATH=\"$PATH:$HOME/.local/bin\"\nexport PATH\nexec {cmd} pdk-models");
    for a in args {
        script += &format!(" '{}'", a.replace('\'', "'\\''"));
    }
    script.push('\n');
    let fail = |why: String| {
        eprintln!("schemify-interop: pdk-models: {host}: {why}");
        (String::new(), 2)
    };
    let mut child = match Command::new("ssh")
        .args([host, "sh", "-s"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail(format!("ssh: {e}")),
    };
    // The script is far below a pipe's buffer: written whole before the report is read.
    let fed = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(script.as_bytes());
    let out = match (fed, child.wait_with_output()) {
        (Ok(()), Ok(o)) => o,
        (Err(e), _) | (_, Err(e)) => return fail(format!("ssh: {e}")),
    };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    match out.status.code() {
        Some(c @ 0..=2) => (text, c as u8),
        _ => {
            print!("{text}");
            fail(format!("the conversion did not finish ({})", out.status))
        }
    }
}

/// Runs `pdk-remote-models` on `args`; its exit status: 0 `PDK_MODELS` written, 1 written
/// and the conversion has notes, 2 nothing written (why on stderr).
pub(super) fn pdk_remote_models(args: &[String]) -> u8 {
    match remote_models(args) {
        Ok((line, code)) => {
            println!("{line}");
            code
        }
        Err(why) => {
            eprintln!("schemify-interop: pdk-remote-models: {why}");
            2
        }
    }
}

/// The outcome line and the conversion's status, or why `schemify.project` is unchanged.
fn remote_models(args: &[String]) -> Result<(String, u8), String> {
    let (project, rest) = args.split_first().ok_or(REMOTE_USAGE)?;
    let mut opt = std::collections::BTreeMap::new();
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match (a.as_str(), it.next()) {
            (k @ ("--host" | "--family" | "--to" | "-o"), Some(v)) => opt.insert(k, v.as_str()),
            _ => return Err(REMOTE_USAGE.into()),
        };
    }
    let host = *opt.get("--host").ok_or(REMOTE_USAGE)?;
    let root = Path::new(project);
    let read = |f: &str| std::fs::read_to_string(root.join(f)).map_err(|e| format!("{f}: {e}"));

    // `remote <family> <[user@]host:/path> models <relpath> dialect <d> [section <s>]`.
    let remotes = read("pdks.remote")?;
    let rows: Vec<Vec<&str>> = remotes
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|w| w.first() == Some(&"remote"))
        .collect();
    let row = match (opt.get("--family"), &rows[..]) {
        (Some(f), _) => rows
            .iter()
            .find(|w| w.get(1) == Some(f))
            .ok_or(format!("pdks.remote has no family {f}"))?,
        (None, [one]) => one,
        (None, _) => {
            return Err(format!(
                "pdks.remote names {} kits: --family picks one",
                rows.len()
            ));
        }
    };
    let [_, family, dest, "models", models, "dialect", dialect, ..] = row[..] else {
        return Err(
            "pdks.remote: expected remote <family> <[user@]host:/path> models <path> dialect <d>"
                .into(),
        );
    };
    let (login, kit) = dest
        .split_once(':')
        .ok_or(format!("pdks.remote: {dest} is no <[user@]host:/path>"))?;
    if login != host {
        return Err(format!(
            "pdks.remote keeps {family} on {login}, not on {host}"
        ));
    }
    // The family names a directory on the host.
    if family.is_empty()
        || !family
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(format!("pdks.remote: {family} is no family name"));
    }

    let manifest = read("schemify.project")?;
    let to = match opt.get("--to") {
        Some(s) => (*s).to_owned(),
        None => pinned(root, &manifest)
            .ok_or("the project's tech map pins no simulator: --to names one")?,
    };
    let out = match opt.get("-o") {
        Some(o) => (*o).to_owned(),
        None => format!(".cache/schemify/pdk-models/{family}/{to}"),
    };
    let convert = [
        "--root", kit, "--entry", models, "--from", dialect, "--to", &to, "-o", &out,
    ];
    let (report, code) = on_host(host, &convert.map(String::from));
    // Only counts of the report are shown: its `diag` lines name the kit's files.
    let notes = report.lines().filter(|l| l.starts_with("diag ")).count();
    let path = match (code, report.lines().find_map(|l| l.strip_prefix("models "))) {
        (0 | 1, Some(p)) => p,
        (0 | 1, None) => {
            return Err(format!(
                "update schemify-interop on {host}: its report names no models"
            ));
        }
        _ if notes > 0 => {
            return Err(format!(
                "nothing converted for {to} ({notes} diagnostics on {host})"
            ));
        }
        _ => {
            return Err(format!(
                "nothing converted for {to}: {}",
                report.lines().next().unwrap_or("no report")
            ));
        }
    };
    // A bare word of the manifest, as the kit's own paths are.
    let word = |c: u8| c.is_ascii_alphanumeric() || b"._+-/".contains(&c);
    if !path.starts_with('/') || !path.bytes().all(word) || path.split('/').any(|s| s == "..") {
        return Err(format!(
            "{host} answered a models path schemify.project cannot hold"
        ));
    }

    let set = format!("set PDK_MODELS {path}");
    let mut lines: Vec<&str> = manifest.lines().collect();
    let ours = |l: &&str| l.split_whitespace().take(2).eq(["set", "PDK_MODELS"]);
    match lines.iter().position(ours) {
        Some(i) => lines[i] = &set,
        None => lines.push(&set),
    }
    let file = root.join("schemify.project");
    let tmp = root.join("schemify.project.tmp");
    std::fs::write(&tmp, lines.join("\n") + "\n")
        .and_then(|()| std::fs::rename(&tmp, &file))
        .map_err(|e| format!("schemify.project: {e}"))?;
    let notes = match notes {
        0 => String::new(),
        n => format!(", {n} notes"),
    };
    Ok((
        format!("{family}: PDK_MODELS is {path} (for {to}, on {host}{notes})"),
        code,
    ))
}

/// The `simulator` row of the tech map the manifest's `lib tech map <path>` names: a row
/// indented under the map's `schemify 1 techmap` line, before its first record.
fn pinned(root: &Path, manifest: &str) -> Option<String> {
    let map = manifest.lines().find_map(|l| {
        let w: Vec<&str> = l.split_whitespace().collect();
        matches!(w[..], ["lib", "tech", "map", _]).then(|| w[3].to_owned())
    })?;
    let text = std::fs::read_to_string(root.join(map)).ok()?;
    let mut lines = text.lines();
    if !lines
        .next()?
        .split_whitespace()
        .eq(["schemify", "1", "techmap"])
    {
        return None;
    }
    lines
        .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t', '#']))
        .find_map(|l| match l.split_whitespace().collect::<Vec<_>>()[..] {
            ["simulator", s] => Some(s.to_owned()),
            _ => None,
        })
}

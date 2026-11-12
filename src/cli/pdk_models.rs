//! `pdk-models`: SpiceRack's model conversion (`crate::pdk_models`), its report printed.
//!
//! `--host <ssh name>` converts a kit where it lives, for one that must not leave its host:
//! the `schemify-interop` there runs the same conversion (one `ssh <host> sh -s`, whatever
//! the login shell), so `--root` and `-o` are directories on the host and only the report
//! comes back. Before its `end`, a `models <path>` line per entry names the converted file
//! on the host: what `set PDK_MODELS` takes for the simulator `--to` names. A kit already in
//! that dialect is not copied: its `models` lines name the kit's own files and no ssh runs.
//! `SFY_INTEROP` is the command on the host (default `schemify-interop`, from the login
//! `PATH` or `~/.local/bin`).

use std::io::Write;
use std::process::{Command, Stdio};

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

/// `pdk-models <args>` run on `host`: its report with the `models` lines, and its status
/// (2 too when ssh or the command there fails).
fn on_host(host: &str, args: &[String]) -> (String, u8) {
    let r = match crate::pdk_models::request(args) {
        Ok(r) => r,
        Err(e) => return e,
    };
    // A name ssh would read as an option (`-oProxyCommand=…`) is no host.
    if host.starts_with('-') {
        return (format!("no ssh host name: {host}\n"), 2);
    }
    let at = |dir: &str| -> String {
        let dir = dir.trim_end_matches('/');
        r.entries
            .iter()
            .map(|e| format!("models {dir}/{e}\n"))
            .collect()
    };
    if r.from == r.to {
        return (format!("converted 0 file(s)\n{}end\n", at(r.root)), 0);
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
    let text = String::from_utf8_lossy(&out.stdout);
    match (out.status.code(), text.strip_suffix("end\n")) {
        (Some(c @ (0 | 1)), Some(body)) => (format!("{body}{}end\n", at(r.out)), c as u8),
        (Some(2), _) => (text.into_owned(), 2),
        _ => {
            print!("{text}");
            fail(format!("the conversion did not finish ({})", out.status))
        }
    }
}

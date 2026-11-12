//! `schemify-interop pdk-models` (feature `cli`): a local kit's model files converted from one
//! simulator dialect to another through SpiceRack's transpiler (`import::kit` reads the kit,
//! `render::render_kit` writes each file). The store is the kit's own source files and their
//! read order; PDK-Manager writes the dialect a simulator wants from them on demand.
//!
//! ```text
//! pdk-models [--host <ssh name>] --root <dir> --entry <relpath>... --from <d> --to <d> -o <outdir>
//!   d: ngspice (or spice) | espice | xyce | ltspice | hspice | spectre | vacask
//! ```
//!
//! A dialect's name is SpiceRack's, which is the name of the Schemify simulator that reads
//! it: `--to` takes the selected simulator (`schemify run`'s) as it is.
//!
//! Every file the entries reach (`--entry` repeats: a kit with a file per corner; includes,
//! `.lib` calls and `include … section=`, at the top, in sections, subcircuits and branches)
//! is converted on its own and written at the same relative path under `<outdir>`, so the
//! references between files still resolve; Verilog-A sources (`ahdl_include`) are copied.
//! Output, files in the kit's read order:
//!
//! ```text
//! converted <n> file(s)           exit 0 or 1; on exit 2 `not written` instead
//! diag <relpath> <code> <count> [<word>...]   per file, per SpiceRack diagnostic code; a
//!                                 refusal (`W`) adds its statements' first words (never the cards)
//! end
//! ```
//!
//! A file the kit lacks is `diag <its relpath> R0006 1` (not repeated on its includer).
//! `diag::info(code)` in SpiceRack is a code's title and explanation. Exit 0: lossless (no
//! `diag` line); 1: written, with notes or missing files; 2: nothing written, either a file
//! the target refuses (its `W` codes among its `diag` lines) or a path outside the root
//! (`R0007`, also printed as `<relpath>: outside the root, refused`). Usage errors print the
//! usage and exit 2.
//!
//! A remote kit never comes here: `--host` (`src/cli/pdk_models.rs`) converts it where it
//! is, with the `schemify-interop` there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::{self, Diags};
use spicerack::syntax::resolve::{FileId, Form, Kit, LoadError, Role};

use crate::spice;

const USAGE: &str = "usage: schemify-interop pdk-models [--host <ssh name>] --root <dir> \
     --entry <relpath>... --from <d> --to <d> -o <outdir>   \
     (d: ngspice | spice | espice | xyce | ltspice | hspice | spectre | vacask)";

/// The model dialect `name` selects: SpiceRack's name for it, `spice` for ngspice. No kit
/// has CDL or Verilog models.
fn dialect(name: &str) -> Result<Dialect, String> {
    let d = match name {
        "spice" => Some(Dialect::Ngspice),
        n => Dialect::from_backend_name(n),
    };
    d.filter(|d| !matches!(d, Dialect::Cdl | Dialect::Verilog))
        .ok_or_else(|| format!("unknown dialect {name}\n{USAGE}"))
}

/// What `pdk-models` was asked for: the paths as given.
pub struct Request<'a> {
    /// `--root`: the directory that holds every file of the kit.
    pub root: &'a str,
    /// `--entry`, each relative to the root.
    pub entries: Vec<&'a str>,
    /// `--from`: the kit's dialect.
    pub from: Dialect,
    /// `--to`: the dialect to write.
    pub to: Dialect,
    /// `-o`: where the converted tree goes.
    pub out: &'a str,
}

/// Reads `args` (after the subcommand); the usage or the unknown dialect, and status 2,
/// when they do not make a request.
pub fn request(args: &[String]) -> Result<Request<'_>, (String, u8)> {
    let mut opt = BTreeMap::new();
    let mut entries = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match (a.as_str(), it.next()) {
            ("--entry", Some(v)) => entries.push(v.as_str()),
            (k @ ("--root" | "--from" | "--to" | "-o"), Some(v)) => {
                opt.insert(k, v.as_str());
            }
            _ => return Err((format!("{USAGE}\n"), 2)),
        }
    }
    let [Some(root), Some(from), Some(to), Some(out)] =
        ["--root", "--from", "--to", "-o"].map(|k| opt.get(k).copied())
    else {
        return Err((format!("{USAGE}\n"), 2));
    };
    if entries.is_empty() {
        return Err((format!("{USAGE}\n"), 2));
    }
    let (from, to) = match (dialect(from), dialect(to)) {
        (Ok(f), Ok(t)) => (f, t),
        (Err(e), _) | (_, Err(e)) => return Err((format!("{e}\n"), 2)),
    };
    // The kit is read as text of its family (ESPice reads ngspice decks).
    if spicerack::syntax::lexicon::of(from.family()).is_none() {
        return Err((
            format!("no reader for --from {}\n{USAGE}\n", from.name()),
            2,
        ));
    }
    Ok(Request {
        root,
        entries,
        from,
        to,
        out,
    })
}

/// Runs `pdk-models` on `args` (after the subcommand): stdout's text and the exit status.
pub fn run(args: &[String]) -> (String, u8) {
    let r = match request(args) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let Ok(root) = std::fs::canonicalize(r.root).map(|p| spice::normalize(&p)) else {
        return (format!("cannot open root {}\n", r.root), 2);
    };
    let entries: Vec<PathBuf> = r.entries.iter().map(|e| root.join(e)).collect();
    convert(&root, &entries, r.from.family(), r.to, Path::new(r.out))
}

/// Converts every file the `entries` reach under `root` into `out`: the report and the exit
/// status. Every file is read and written to text before any is stored, so a refusal leaves
/// `out` untouched.
fn convert(
    root: &Path,
    entries: &[PathBuf],
    from: Dialect,
    to: Dialect,
    out: &Path,
) -> (String, u8) {
    // A lexical path under the root whose real path (symlinks resolved) is also under it.
    let mut load = |p: &Path| {
        if !p.starts_with(root) {
            return Err(LoadError::Refused);
        }
        let real = std::fs::canonicalize(p).map_err(|_| LoadError::NotFound)?;
        if !real.starts_with(root) {
            return Err(LoadError::Refused);
        }
        let bytes = std::fs::read(&real).map_err(|_| LoadError::NotFound)?;
        Ok(spicerack::import::decode(&bytes))
    };
    let kit = Kit::load(entries, Form::Library, from, &mut load);
    let rel = |p: &Path| {
        let p = p.strip_prefix(root).unwrap_or(p);
        p.to_string_lossy().replace('\\', "/")
    };
    let (mut report, mut done) = (String::new(), Vec::new());
    let (mut refused, mut outside) = (false, String::new());
    for (i, file) in kit.files().iter().enumerate() {
        let shown = rel(&file.path);
        // The first word of each refused statement (W codes): what to translate by hand,
        // never the cards (live realkit: 32 files refused W0001, by keyword).
        let lex = spicerack::syntax::lexicon::of(kit.dialect()).expect("a dialect with a reader");
        let toks = spicerack::syntax::tokenize(&file.text, lex);
        let word = |t: u32| -> String {
            let at = toks.start(t.min(toks.len().saturating_sub(1) as u32)) as usize;
            let line_start = file.text[..at].rfind('\n').map_or(0, |i| i + 1);
            file.text[line_start..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || "._".contains(*c))
                .collect()
        };
        let codes = match file.role {
            Role::Unread(LoadError::Refused) => {
                refused = true;
                outside += &format!("{shown}: outside the root, refused\n");
                vec![(Code::R0007, String::new())]
            }
            Role::Unread(LoadError::NotFound) => vec![(Code::R0006, String::new())],
            Role::VerilogA => {
                done.push((&file.path, file.text.clone()));
                Vec::new()
            }
            Role::Deck | Role::Library => {
                match render::render_kit(&kit, FileId(i as u32), &shown, to) {
                    Ok(w) => {
                        done.push((&file.path, w.text));
                        codes(&w.notes, &word)
                    }
                    Err(d) => {
                        refused = true;
                        codes(&d, &word)
                    }
                }
            }
        };
        let mut counts: BTreeMap<Code, (usize, Vec<String>)> = BTreeMap::new();
        // An unread include is reported at its own path, not again on its includer's statement.
        let own = matches!(file.role, Role::Unread(_));
        for (c, w) in codes {
            if own || !matches!(c, Code::R0006 | Code::R0007) {
                let e = counts.entry(c).or_default();
                e.0 += 1;
                if !w.is_empty() && !e.1.contains(&w) {
                    e.1.push(w);
                }
            }
        }
        for (c, (n, words)) in counts {
            let words = if words.is_empty() {
                String::new()
            } else {
                format!(" {}", words.join(" "))
            };
            report += &format!("diag {shown} {c:?} {n}{words}\n");
        }
    }
    if refused {
        return (format!("not written\n{outside}{report}end\n"), 2);
    }
    for (p, text) in &done {
        let to = out.join(p.strip_prefix(root).unwrap_or(p));
        let wrote = to
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&to, text));
        if let Err(e) = wrote {
            return (format!("{}: {e}\n", to.display()), 2);
        }
    }
    let lossy = !report.is_empty();
    (
        format!("converted {} file(s)\n{report}end\n", done.len()),
        u8::from(lossy),
    )
}

/// Each diagnostic's code, with its statement's first word when it is a refusal (`W`).
fn codes(d: &Diags, word: &dyn Fn(u32) -> String) -> Vec<(Code, String)> {
    d.diags
        .iter()
        .map(|d| {
            let w = if format!("{:?}", d.code).starts_with('W') {
                word(d.token)
            } else {
                String::new()
            };
            (d.code, w)
        })
        .collect()
}

#!/usr/bin/env bash
# The local entry point for work on the Virtuoso host ($SFY_HOST, its ssh name, required):
#
#   SFY_HOST=<host> live/host.sh sync          the tracked tree at HEAD and its vendored
#                                              dependencies to ~/sfytest/head/src (and
#                                              live/local, when there is one)
#   SFY_HOST=<host> live/host.sh build         cargo build --release --features cli there,
#                                              offline; installs ~/.local/bin/schemify-interop
#   SFY_HOST=<host> live/host.sh kit [flags…]  live/test.sh in ~/sfytest/head/work with
#                                              --cdslib ~/sfytest/cds.lib --env ~/sfytest/env.csh
#   SFY_HOST=<host> live/host.sh check <name>  live/checks/run.sh <name> with the installed CLI,
#                                              its work directory ~/sfytest/head/checks
#
# One ssh at a time: each subcommand holds a local lock for its whole run and exits 2 when
# another holds it. The host's login shell is csh, so the remote half is a script on the
# stdin of `ssh … sh -s`. Heavy steps wait while the host's 1-minute load is 16 or more and
# run under nice 19 with SFY_HOST_JOBS cargo jobs (default 4).
#
# kit fetches out/report.md and summary.txt into $SFY_HOST_OUT (default a fresh temporary
# directory). A flag that brings in a PDK or a user's design (--pdk --lib --cell --callbacks
# --project) can put PDK names in them: then it fetches nothing and prints only the counts.
#
# SFY_HOST_LOCAL: the site-local checks sync ships (default live/local of the main checkout;
# never in git). SFY_HOST_VENDOR: the local `cargo vendor` cache (default ~/.cache/sfy-host).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
host="${SFY_HOST:-}"
[[ -n $host ]] || { echo "usage: SFY_HOST=<host> $0 sync | build | kit [flags…] | check <name>" >&2; exit 2; }
lock="${XDG_RUNTIME_DIR:-/tmp}/sfy-host.lock"
exec 9>"$lock"
flock -n 9 || { echo "host.sh: $lock is held: another host.sh is talking to $host" >&2; exit 2; }

ssh_=(ssh -x -o BatchMode=yes "$host")
# The remote script on stdin, in ~/sfytest/head with rustup's cargo and the job limit.
remote() {
  { printf 'set -e\nexport PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH" CARGO_BUILD_JOBS=%s\n' "${SFY_HOST_JOBS:-4}"
    printf 'mkdir -p "$HOME/sfytest/head" && cd "$HOME/sfytest/head"\n'
    cat; } | "${ssh_[@]}" sh -s
}
calm='while [ "$(cut -d. -f1 /proc/loadavg)" -ge 16 ]; do echo "host.sh: load $(cut -d" " -f1 /proc/loadavg), waiting"; sleep 60; done'

case "${1:-}" in
  sync)
    cache="${SFY_HOST_VENDOR:-$HOME/.cache/sfy-host}"
    main="$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)"
    local_dir="${SFY_HOST_LOCAL:-${main%/.git}/live/local}"
    # The host cannot fetch the private git dependency: the host copy builds from vendored
    # sources only (offline), into ~/sfytest/head/target so a new sync keeps the build.
    mkdir -p "$cache/src/.cargo"
    { (cd "$repo" && cargo vendor --locked "$cache/vendor" 2> "$cache/vendor.log") \
      || { cat "$cache/vendor.log" >&2; exit 1; }; } \
      | sed 's|^directory = .*|directory = "../vendor"|' > "$cache/src/.cargo/config.toml"
    printf '\n[net]\noffline = true\n\n[build]\ntarget-dir = "../target"\n' >> "$cache/src/.cargo/config.toml"
    remote <<< 'rm -rf src vendor'
    # The one remote command that is no `sh -s` script: its stdin is the archives (-i reads
    # them one after another; -m dates the files now, so cargo rebuilds what changed).
    { git -C "$repo" archive --prefix=src/ HEAD
      tar -c -f - -C "$cache" vendor src/.cargo
      [[ ! -d $local_dir ]] || tar -c -f - -C "$local_dir" --transform='s,^\.,src/live/local,' .
    } | "${ssh_[@]}" tar -x -m -i -f - -C sfytest/head
    echo "host.sh: $host:sfytest/head/src is $(git -C "$repo" rev-parse --short HEAD)$([[ -d $local_dir ]] && echo ", with $local_dir")"
    ;;
  build)
    remote <<EOF
$calm
cd src
nice -n 19 cargo build --release --locked --offline --features cli -j "\$CARGO_BUILD_JOBS"
mkdir -p "\$HOME/.local/bin"
install -m 755 ../target/release/schemify-interop "\$HOME/.local/bin/schemify-interop"
echo "host.sh: installed \$HOME/.local/bin/schemify-interop"
EOF
    ;;
  kit)
    shift
    pdk=0 args=""
    (($#)) && args="$(printf '%q ' "$@")"
    for a in "$@"; do case $a in --pdk|--lib|--cell|--callbacks|--project) pdk=1 ;; esac; done
    # The run lock is checks/run.sh's: one Virtuoso run at a time (test.sh clears /tmp/sfy_*).
    status=0
    remote <<EOF || status=$?
$calm
s=0
flock /tmp/\$(id -un)-sfy-run.lock nice -n 19 bash src/live/test.sh --work "\$HOME/sfytest/head/work" \
  --cdslib "\$HOME/sfytest/cds.lib" --env "\$HOME/sfytest/env.csh" $args> kit.log 2>&1 || s=\$?
echo "host.sh: test.sh exit \$s (log: ~/sfytest/head/kit.log, sfylive-run.log)"
r=work/out/report.md
if [ -f \$r ]; then
  [ $pdk = 1 ] || grep '^Verdict:' \$r
  for w in PASS FAIL WARN SKIP INFO; do printf '%s %s  ' \$w "\$(grep -c "^| [^|]* | \$w |" \$r)"; done; echo
else
  echo "host.sh: no report"
fi
exit \$s
EOF
    if [[ $pdk == 0 ]]; then
      dest="${SFY_HOST_OUT:-$(mktemp -d)}"
      mkdir -p "$dest"
      remote <<< 'cd work && tar -c -f - out/report.md summary.txt 2>/dev/null' | tar -x -f - -C "$dest" \
        || echo "host.sh: the fetch was incomplete" >&2
      echo "host.sh: $dest/out/report.md, $dest/summary.txt"
    fi
    exit "$status"
    ;;
  check)
    [[ -n ${2:-} ]] || { echo "usage: SFY_HOST=<host> $0 check <name>" >&2; exit 2; }
    remote <<EOF
$calm
w="\$HOME/sfytest/head/checks"
mkdir -p "\$w"
[ -f "\$w/cds.lib" ] || cp "\$HOME/sfytest/cds.lib" "\$w/cds.lib"
schemify-interop install-skill "\$w" > /dev/null
SFYCHECK_WORK="\$w" SFYCHECK_CLI="\$HOME/.local/bin/schemify-interop" nice -n 19 bash src/live/checks/run.sh $(printf '%q' "$2")
EOF
    ;;
  *) echo "usage: SFY_HOST=<host> $0 sync | build | kit [flags…] | check <name>" >&2; exit 2 ;;
esac

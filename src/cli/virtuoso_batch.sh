# schemify-interop's batch Virtuoso session (pdk-map, virtuoso-pull, virtuoso-push): run on
# the host as `sh -s`, whatever the login shell, below the settings, sfy_cmd, sfy_load and
# sfy_files() the command writes above this line. Starts Virtuoso in batch on run.il, which
# loads schemify.il and runs one entry point on run.sfyx: a dump it writes, or with sfy_load
# set the file it loads. The Cadence environment comes as live/checks/run.sh gets it: the
# setup script SFY_CADENCE_ENV sourced through csh (through sh when it ends in .sh) before
# Virtuoso starts, then CDS_LIC_FILE and CDS_LIC_ONLY as given (SFY_CDS_LIC_*), then
# $VIRTUOSO (default virtuoso from PATH) runs in SFY_CDS_DIR (default the login directory),
# whose cds.lib must define the libraries, for at most SFY_VIRTUOSO_TIMEOUT seconds (default
# 1800). Exit status: 0 the entry point passed, its dump (with sfy_load, its log) on stdout;
# 1 it refused something, its log on stdout; 3 Virtuoso did not start or did not finish (the
# variables to set on stderr).
umask 077
sfy_fail() {
  echo "$sfy_cmd: $1. Set, where $sfy_cmd runs: VIRTUOSO (the Virtuoso command), SFY_CADENCE_ENV (the Cadence or kit setup script on the host), CDS_LIC_FILE or CDS_LIC_ONLY (the licence), SFY_CDS_DIR (the host directory whose cds.lib defines the libraries)${2:+. The session log stays at $2 on the host}" >&2
  if [ -z "${2:-}" ] && [ -n "${sfy_t:-}" ]; then rm -rf "$sfy_t"; fi
  exit 3
}
case "${SFY_CDS_DIR:-.}" in /*) ;; *) SFY_CDS_DIR="$PWD/${SFY_CDS_DIR:-.}" ;; esac
export SFY_CDS_DIR
sfy_t=$(mktemp -d "${TMPDIR:-/tmp}/sfy-run.XXXXXX") || sfy_fail "cannot make a temporary directory"
SFY_RUN_DIR=$sfy_t
export SFY_RUN_DIR
sfy_files
cat >"$sfy_t/virtuoso.sh" <<'SFY_RUN_END'
if [ -n "${SFY_CDS_LIC_FILE:-}" ]; then CDS_LIC_FILE=$SFY_CDS_LIC_FILE; export CDS_LIC_FILE; fi
if [ -n "${SFY_CDS_LIC_ONLY:-}" ]; then CDS_LIC_ONLY=$SFY_CDS_LIC_ONLY; export CDS_LIC_ONLY; fi
cd "$SFY_CDS_DIR" || exit 4
set -- $VIRTUOSO
command -v "$1" >/dev/null 2>&1 || exit 5
if command -v timeout >/dev/null 2>&1; then set -- timeout "${SFY_VIRTUOSO_TIMEOUT:-1800}" "$@"; fi
"$@" -nograph -restore "$SFY_RUN_DIR/run.il" -log "$SFY_RUN_DIR/CDS.log" </dev/null >/dev/null 2>&1
SFY_RUN_END
case "${SFY_CADENCE_ENV:-}" in
  "") sh "$sfy_t/virtuoso.sh" ;;
  *)
    [ -r "$SFY_CADENCE_ENV" ] || sfy_fail "cannot read SFY_CADENCE_ENV ($SFY_CADENCE_ENV)"
    export SFY_CADENCE_ENV
    case "$SFY_CADENCE_ENV" in
      *.sh) (. "$SFY_CADENCE_ENV" >/dev/null 2>&1; sh "$sfy_t/virtuoso.sh") ;;
      *)
        command -v csh >/dev/null 2>&1 || sfy_fail "no csh to source SFY_CADENCE_ENV with"
        csh -f -c 'source "$SFY_CADENCE_ENV" >& /dev/null; exec sh "$SFY_RUN_DIR/virtuoso.sh"' ;;
    esac ;;
esac
sfy_st=$?
case $sfy_st in
  4) sfy_fail "cannot enter SFY_CDS_DIR ($SFY_CDS_DIR)" ;;
  5) sfy_fail "no Virtuoso command \`${VIRTUOSO%% *}\` on PATH after the setup" ;;
  124) sfy_fail "Virtuoso ran out of its SFY_VIRTUOSO_TIMEOUT" "$sfy_t/CDS.log" ;;
esac
# The log's last line is PASS or FAIL once the entry point has finished.
case $(tail -n 1 "$sfy_t/run.sfyx.log" 2>/dev/null) in
  PASS)
    if [ -n "${sfy_load:-}" ]; then cat "$sfy_t/run.sfyx.log"; else cat "$sfy_t/run.sfyx"; fi
    rm -rf "$sfy_t"
    exit 0 ;;
  FAIL)
    cat "$sfy_t/run.sfyx.log"
    rm -rf "$sfy_t"
    exit 1 ;;
esac
sfy_fail "Virtuoso did not finish its run (exit $sfy_st)" "$sfy_t/CDS.log"

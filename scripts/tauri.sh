#!/bin/sh
# `pnpm tauri …`: runs the Tauri CLI, then trims src-tauri/target/ so builds don't pile up.
# Cargo never deletes old artifacts (each version bump leaves a full copy); cargo-sweep drops the oldest
# until the folder is under the cap, keeping what recent builds reuse. Installed in .tools/ (see README).
trap 'true' INT TERM # Ctrl-C stops tauri, not this script, so the sweep still runs
tauri "$@"
code=$?
sweep=.tools/bin/cargo-sweep
[ -x "$sweep" ] && "$sweep" sweep --maxsize 6GB src-tauri >/dev/null 2>&1
exit $code

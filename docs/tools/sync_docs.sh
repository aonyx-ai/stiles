#!/bin/bash
# Pull the docs desk's edits into the working tree.
#
# This exports the desk's store with desk.py, overlays it on a fresh parse of
# the tree, and writes back only the pages that carry edits. Nothing is
# written to the desk, so a sync never races a live editor.
#
# Pass --check to see what would change without writing anything.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="${WORK:-${TMPDIR:-/tmp}/docs-desk}"
PULL="${PULL:-$WORK/pull}"
mkdir -p "$WORK"

python3 "$HERE/desk.py" export "$PULL"
python3 "$HERE/parse_docs.py" > "$WORK/seed.json"
python3 "$HERE/merge_docs.py" "$WORK/seed.json" "$PULL" > "$WORK/edited.json"
python3 "$HERE/write_docs.py" "$@" "$WORK/edited.json"

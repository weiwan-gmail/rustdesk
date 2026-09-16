#!/usr/bin/env bash
# Usage: scripts/compare-upstream-master.sh [ours-ref] [theirs-ref]
# Prints ahead/behind and content-conflict paths from git merge-tree.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

OURS="${1:-HEAD}"
THEIRS="${2:-upstream/master}"

if ! git rev-parse --verify "$THEIRS" >/dev/null 2>&1; then
  echo "missing $THEIRS; fetch with: git fetch https://github.com/rustdesk/rustdesk.git master:refs/remotes/upstream/master" >&2
  exit 1
fi

BASE="$(git merge-base "$OURS" "$THEIRS")"
AHEAD="$(git rev-list --count "$THEIRS".."$OURS")"
BEHIND="$(git rev-list --count "$OURS".."$THEIRS")"

echo "ours:     $(git rev-parse --short "$OURS") $(git log -1 --format='%s' "$OURS")"
echo "theirs:   $(git rev-parse --short "$THEIRS") $(git log -1 --format='%s' "$THEIRS")"
echo "base:     $(git rev-parse --short "$BASE") $(git log -1 --format='%ci %s' "$BASE")"
echo "ahead:    $AHEAD"
echo "behind:   $BEHIND"

echo
echo "content conflicts:"
set +e
TREE_OUT="$(git merge-tree --write-tree --name-only "$OURS" "$THEIRS" 2>&1)"
TREE_RC=$?
set -e
# First line is the tree OID when the merge is clean, or a conflict list then a blank line.
printf '%s\n' "$TREE_OUT" | awk '
  /^[0-9a-f]{40}$/ { next }
  /^$/ { exit }
  { print "  " $0 }
'
echo "merge-tree-exit: $TREE_RC"

#!/usr/bin/env bash
# Stop hook — remind to update CHANGELOG if any crate or app source changed.
set -u
if git -C "$(pwd)" diff --quiet HEAD 2>/dev/null; then
  exit 0
fi
changed=$(git -C "$(pwd)" diff --name-only HEAD 2>/dev/null || true)
if echo "$changed" | grep -qE '^(crates/|apps/|cli/|packages/)'; then
  if ! echo "$changed" | grep -q '^CHANGELOG.md$'; then
    cat >&2 <<EOF
REMINDER: Source changed but CHANGELOG.md not updated.
          If this is user-visible, add a bullet under [Unreleased] before commit.
EOF
  fi
fi
exit 0

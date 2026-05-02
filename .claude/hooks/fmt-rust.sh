#!/usr/bin/env bash
# PostToolUse hook — auto-format any .rs file touched by Write or Edit.
set -u
path=$(jq -r '.tool_input.file_path // empty' 2>/dev/null || true)
if [[ "$path" == *.rs && -f "$path" ]]; then
  rustfmt --edition 2021 "$path" 2>/dev/null || true
fi
exit 0

#!/usr/bin/env bash
# PostToolUse hook — warn (non-blocking) if core library crates gain I/O imports.
set -u
path=$(jq -r '.tool_input.file_path // empty' 2>/dev/null || true)
if [[ "$path" == *crates/*-core/*.rs && -f "$path" ]]; then
  if grep -nE 'std::fs|std::net|std::process|tokio::fs|tokio::net|tokio::process|reqwest::|ureq::|git2::|dirs::' "$path"; then
    cat >&2 <<EOF
WARNING: core-crate violation in $path
         A crate named *-core is expected to be I/O-free.
         Move std::fs / std::net / std::process / tokio::{fs,net,process}
         / reqwest / ureq / git2 / dirs to an I/O-side crate.
EOF
  fi
fi
exit 0

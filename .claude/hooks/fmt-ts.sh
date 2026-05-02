#!/usr/bin/env bash
# PostToolUse hook — auto-format TS/JS/TSX/JSX files via Prettier if available.
set -u
path=$(jq -r '.tool_input.file_path // empty' 2>/dev/null || true)
case "$path" in
  *.ts|*.tsx|*.js|*.jsx|*.mjs|*.cjs|*.json|*.md)
    if [[ -f "$path" ]] && command -v prettier >/dev/null 2>&1; then
      prettier --write "$path" 2>/dev/null || true
    fi
    ;;
esac
exit 0

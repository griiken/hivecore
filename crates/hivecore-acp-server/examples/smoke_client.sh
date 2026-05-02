#!/usr/bin/env bash
# Smoke-test the hivecore-acp server by sending a minimal ACP handshake +
# prompt over stdio. Reads notifications from the server's stdout.
#
# Run with:
#     OPENAI_API_KEY=... ./examples/smoke_client.sh
set -euo pipefail

BIN="${ACP_BIN:-./target/release/hivecore-acp}"
WORKSPACE="$(mktemp -d)"
echo "workspace: $WORKSPACE" >&2
cat > "$WORKSPACE/hello.txt" <<'EOF'
hello world
EOF

export HIVECORE_WORKSPACE="$WORKSPACE"
export HIVECORE_MODEL="${HIVECORE_MODEL:-gpt-5.4-nano}"

# JSON-RPC requests, line-delimited.
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true,"writeTextFile":true},"terminal":true}}}'
  echo '{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"'"$WORKSPACE"'","mcpServers":[]}}'
  # Read the session id back from line 2 then send prompt — we use a
  # well-known stub session id pattern: pull it from the response in a real
  # client. For smoke we send the same prompt key matching session id "stub"
  # but our server replies with its own UUID id, so this script just shows
  # the wire works for initialize + session/new. A full prompt round-trip is
  # easier to do with the rust integration test in tests/.
} | "$BIN" 2>"$WORKSPACE/stderr.log"

echo "--- stderr ---" >&2
cat "$WORKSPACE/stderr.log" >&2

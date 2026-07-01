#!/usr/bin/env bash
# Local mTLS test server. Requires a client certificate (depth 1) signed by
# auth-cert, so a no-cert request is rejected and our token-backed client must
# present the auth-cert identity. Serves a simple page on GET (s_server -www).
#
# Run ./scripts/setup-test-token.sh first.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$REPO_ROOT/.devcontainer/test-harness"

if [[ ! -f "$WORK/server.crt.pem" ]]; then
  echo "error: harness not provisioned; run ./scripts/setup-test-token.sh first" >&2
  exit 1
fi

echo "Serving https://localhost:8443 (Ctrl-C to stop) — client cert required"
exec openssl s_server -accept 8443 \
  -cert "$WORK/server.crt.pem" -key "$WORK/server.key.pem" \
  -CAfile "$WORK/auth-cert.crt.pem" -Verify 1 -www

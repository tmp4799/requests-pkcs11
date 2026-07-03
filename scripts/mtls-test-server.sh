#!/usr/bin/env bash
# Local mTLS test server. Requires a client certificate signed by auth-cert,
# so a no-cert request is rejected and our token-backed client must present
# the auth-cert identity. Returns JSON: GET describes the request, POST also
# echoes the body (so client method/body handling can be verified).
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
exec python3 - "$WORK" <<'EOF'
import http.server, json, ssl, sys

class Handler(http.server.BaseHTTPRequestHandler):
    def _reply(self, obj):
        body = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self._reply({"method": "GET", "path": self.path})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        echo = self.rfile.read(length).decode("utf-8", "replace")
        self._reply({"method": "POST", "path": self.path, "echo": echo})

work = sys.argv[1]
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(f"{work}/server.crt.pem", f"{work}/server.key.pem")
ctx.load_verify_locations(f"{work}/auth-cert.crt.pem")
ctx.verify_mode = ssl.CERT_REQUIRED  # reject clients without a trusted cert

httpd = http.server.HTTPServer(("0.0.0.0", 8443), Handler)
httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
httpd.serve_forever()
EOF

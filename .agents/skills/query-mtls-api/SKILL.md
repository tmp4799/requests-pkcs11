---
name: query-mtls-api
description: Use when the user asks to query, check, or triage an internal API that requires mTLS client-certificate authentication backed by a PKCS#11 token (CSPid, SoftHSM, smartcard, HSM). Performs the HTTPS request via the requests_pkcs11 Python module and summarizes the result. Trigger on "query the internal API", "hit the mTLS endpoint", "client cert", "PKCS#11", "CSPid", "token-backed TLS".
---

# Query an mTLS API via a PKCS#11 token

Make an HTTPS request whose TLS client certificate + private key live on a
PKCS#11 token (the key never leaves the token), then triage the response.

## Configuration

All environment-specific values come from environment variables — never
hardcode them. Ask the user for any that are missing rather than guessing.

| Variable        | Meaning                                                        |
|-----------------|----------------------------------------------------------------|
| `PKCS11_MODULE` | Path to the PKCS#11 module `.so` (e.g. the CSPid library, or `libsofthsm2.so` for the local test token) |
| `PKCS11_LABEL`  | `CKA_LABEL` of the client certificate + key pair on the token  |
| `PKCS11_CA`     | PEM file of trust anchors for the **server's** certificate     |
| `PKCS11_PIN`    | Token PIN. **Leave unset** for pre-authenticated tokens (e.g. the CSPid agent logs the session in out-of-band) |

For the local SoftHSM test harness (after `./scripts/setup-test-token.sh`),
`SOFTHSM2_CONF` must also point at `.devcontainer/test-harness/softhsm2.conf`,
and the values are: label `auth-cert`, PIN `1234`, CA
`.devcontainer/test-harness/ca.crt.pem`, URL `https://localhost:8443/`.

## Invocation

Run the helper next to this file with a Python that has the `requests_pkcs11`
wheel installed (see the repo README for building it):

```bash
python query.py URL [METHOD [BODY]]
# e.g. python query.py https://localhost:8443/health
#      python query.py https://api.internal/jobs POST '{"action":"retry"}'
```

It prints `HTTP <status>`, the response headers, a blank line, then the raw
body. For one-off calls you can equally call the module directly:

```python
import requests_pkcs11
resp = requests_pkcs11.request(module, label, url, ca, pin=pin)  # pin=None if pre-authenticated
resp.status, resp.headers, resp.text, resp.json()
```

## Triage the result

Report a short verdict, not a dump:

1. **Transport errors** (`RuntimeError` before any `HTTP` line) — the message
   is a context chain; read it right-to-left for the root cause:
   - `certificate labelled "x": not found` → wrong `PKCS11_LABEL` or wrong token/module.
   - `private key labelled "x": not found` (but the certificate WAS found) →
     the token likely needs a PIN: without a login, private objects are
     invisible. Set `PKCS11_PIN`.
   - `PIN is incorrect` → bad `PKCS11_PIN`.
   - certificate verify / `UnknownIssuer` → `PKCS11_CA` does not anchor the server's chain.
   - connection refused / timeout → server down or wrong URL.
2. **HTTP status** — 2xx: proceed to the body. 401/403: the server rejected
   the presented identity (right label? server trusts that cert?). 4xx: bad
   request — show the body. 5xx: server-side failure — show the body.
3. **Body** — if JSON, parse it and summarize the fields that indicate
   health/errors (e.g. `status`, `errors`, counts); quote failing entries
   verbatim. If not JSON, show the first few lines.

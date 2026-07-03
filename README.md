# requests-pkcs11

HTTPS requests authenticated by an mTLS client certificate whose private key
lives on a PKCS#11 token — the key never leaves the token. A Rust core
(`rustls` with a custom signer that delegates TLS signing to the token) is
exposed to Python as the `requests_pkcs11` module via PyO3. Built for
CSPid-style corporate tokens; tested offline against a SoftHSM2 software token.

## Layout

- `crates/core` — PKCS#11 session handling, rustls signer, blocking HTTPS client, and a `spike` CLI.
- `crates/py` — PyO3 wrapper; builds the `requests_pkcs11` Python wheel with maturin.
- `scripts/` — test-token provisioning and a local mTLS echo server.
- `.agents/skills/query-mtls-api/` — Claude Code skill for querying/triaging an mTLS API (mirrored at `.claude/skills/`).

## Prerequisites

The authoritative build/test target is **Linux** (SoftHSM2 is used for offline
testing and the devcontainer provides it). On macOS you can `cargo build` /
`cargo clippy`, but end-to-end runs happen in the container.

- Docker (or a devcontainer-aware editor) — everything else (Rust, SoftHSM2,
  OpenSC, Python, maturin) is inside the image.

## Build

```bash
docker build -t requests-pkcs11-dev .devcontainer
docker run --rm -it -v "$PWD:/workspaces/requests-pkcs11" -w /workspaces/requests-pkcs11 \
  -e SOFTHSM2_CONF=/workspaces/requests-pkcs11/.devcontainer/test-harness/softhsm2.conf \
  requests-pkcs11-dev bash
```

Inside the container (`CARGO_TARGET_DIR=/tmp/target` keeps Linux artifacts out
of a macOS-host `target/`):

```bash
export CARGO_TARGET_DIR=/tmp/target
cargo build --workspace                          # Rust core + PyO3 crate
maturin build --release -m crates/py/Cargo.toml  # Python wheel -> /tmp/target/wheels/
```

Install the wheel into a venv:

```bash
python3 -m venv /tmp/venv
/tmp/venv/bin/pip install /tmp/target/wheels/requests_pkcs11-*.whl
/tmp/venv/bin/python -c 'import requests_pkcs11; print(requests_pkcs11.request)'
```

## Test harness (SoftHSM token + local mTLS server)

```bash
./scripts/setup-test-token.sh        # provision token "testtok" (PIN 1234) + certs
./scripts/mtls-test-server.sh &      # https://localhost:8443, client cert required
```

The token holds two identities — `auth-cert` (RSA, the one the server trusts)
and `sign-cert` (EC decoy) — so selecting the right cert by label is actually
exercised. See `.devcontainer/README.md` for details.

## Usage

```python
import requests_pkcs11

resp = requests_pkcs11.request(
    module="/usr/lib/softhsm/libsofthsm2.so",    # PKCS#11 module (.so)
    label="auth-cert",                           # CKA_LABEL of cert + key on the token
    url="https://localhost:8443/health",
    ca=".devcontainer/test-harness/ca.crt.pem",  # PEM trust anchors for the server
    pin="1234",                                  # None for pre-authenticated tokens
)
print(resp.status)     # 200
print(resp.json())     # parsed body; also .text, .body (bytes), .headers (dict)
```

`method`, `headers` (dict), and `body` (bytes) are optional keyword arguments.
Failures raise `RuntimeError` with a context chain, e.g.
`certificate labelled "nope": not found`.

### Certificate, PIN, and module setup

`request()` needs four things:

- **module** — the vendor's PKCS#11 library path. SoftHSM for tests
  (`libsofthsm2.so`); in production the CSPid `.so`. If the module needs its
  own config (SoftHSM reads `SOFTHSM2_CONF`), set that env var too.
- **label** — the `CKA_LABEL` shared by the certificate object and its private
  key on the token. List objects with
  `pkcs11-tool --module <path-to.so> -O` to find it.
- **pin** — the token user PIN. Pass `None` for tokens a background agent has
  already authenticated (the CSPid agent logs the session in out-of-band; the
  code then skips `C_Login` entirely).
- **ca** — a PEM bundle that anchors the **server's** certificate chain (the
  internal CA for corporate APIs; `ca.crt.pem` for the test server).

Swapping SoftHSM values for the real module path + label is the only change
needed for production.

## Claude Code skill

`.agents/skills/query-mtls-api/` teaches the agent to query an mTLS API and
triage the response. Configure with env vars, then ask Claude to query the
endpoint (or run the helper directly):

```bash
export PKCS11_MODULE=/usr/lib/softhsm/libsofthsm2.so
export PKCS11_LABEL=auth-cert
export PKCS11_PIN=1234                                  # omit for pre-authenticated tokens
export PKCS11_CA=.devcontainer/test-harness/ca.crt.pem
/tmp/venv/bin/python .agents/skills/query-mtls-api/query.py https://localhost:8443/health
```

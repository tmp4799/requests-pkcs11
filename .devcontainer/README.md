# Dev container + test harness

A Linux environment that mirrors the production target and lets us test the full
PKCS#11 mTLS path **offline** — no real CSPID module needed — using a SoftHSM2
software token.

## Build

Open the repo in a devcontainer-aware editor, or build directly:

```bash
docker build -t requests-pkcs11-dev .devcontainer
docker run --rm -it -v "$PWD:/workspaces/requests-pkcs11" -w /workspaces/requests-pkcs11 requests-pkcs11-dev bash
```

## Provision the test token

```bash
./scripts/setup-test-token.sh
```

Creates a SoftHSM2 token (`testtok`, PIN `1234`) holding **two** client identities
so we can test cert selection by label/ID:

| Label       | Key | CKA_ID | Role                                        |
|-------------|-----|--------|---------------------------------------------|
| `auth-cert` | RSA | `01`   | the identity the test server trusts (mirrors CSPid: RSA / TLS 1.3 / RSA-PSS) |
| `sign-cert` | EC  | `02`   | decoy — must NOT be selected; proves label selection across key types |

It also generates a test CA (`ca.crt.pem`) and a `localhost` server leaf
(`server.{crt,key}.pem`) it signs, and prints the on-token objects via
`pkcs11-tool -O`. All output lives under `.devcontainer/test-harness/` (gitignored);
the module path is auto-detected and `SOFTHSM2_CONF` is written there too.

Verify the full path (token-signed mTLS GET):

```bash
export CARGO_TARGET_DIR=/tmp/target PKCS11_MODULE=$(ls /usr/lib/*/softhsm/libsofthsm2.so)
export SOFTHSM2_CONF=$PWD/.devcontainer/test-harness/softhsm2.conf
./scripts/mtls-test-server.sh & sleep 2
cargo run --bin spike -- --label auth-cert --pin 1234 \
  --url https://localhost:8443/ --ca .devcontainer/test-harness/ca.crt.pem
# expect: GET ... -> HTTP 200
```

## Run the mTLS test server

```bash
./scripts/mtls-test-server.sh    # https://localhost:8443, client cert required
```

Rejects requests with no client cert; accepts the `auth-cert` identity. This is the
target our Rust spike (bead `4wt`) hits for an authenticated `200`.

## Going to production

Swap the SoftHSM module path + token label for the real CSPID `.so` and cert label.
Nothing else about the code path changes.

#!/usr/bin/env bash
# Provision a SoftHSM2 token that stands in for the real CSPID PKCS#11 module.
#
# Creates a token holding TWO client identities (auth-cert, sign-cert) so we can
# test selecting the correct cert by label/ID (bead uov) — the real token is
# expected to hold multiple certs. Also generates a server cert for the local
# mTLS test server that trusts the auth-cert identity.
#
# Re-runnable: wipes and recreates the harness each time.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$REPO_ROOT/.devcontainer/test-harness"   # gitignored
TOKENS="$WORK/softhsm-tokens"
export SOFTHSM2_CONF="$WORK/softhsm2.conf"

PIN=1234
SOPIN=123456
MODULE="$(ls /usr/lib/softhsm/libsofthsm2.so /usr/lib/*/softhsm/libsofthsm2.so 2>/dev/null | head -1)"
if [[ -z "${MODULE}" ]]; then
  echo "error: libsofthsm2.so not found (is softhsm2 installed?)" >&2
  exit 1
fi

rm -rf "$WORK"
mkdir -p "$TOKENS"
cat > "$SOFTHSM2_CONF" <<EOF
directories.tokendir = $TOKENS
objectstore.backend = file
log.level = ERROR
EOF

softhsm2-util --init-token --free --label testtok --pin "$PIN" --so-pin "$SOPIN"

# Generate a key + self-signed cert off-token, import the key onto the token,
# then write the matching X.509 cert object. Key and cert share a CKA_ID so
# PKCS#11 pairs them. $3 selects the key type: "rsa" or "ec".
make_identity() {
  local label="$1" id="$2" kind="$3"
  local keyargs
  if [ "$kind" = rsa ]; then
    keyargs="-newkey rsa:2048"
  else
    keyargs="-newkey ec -pkeyopt ec_paramgen_curve:prime256v1"
  fi
  openssl req -x509 $keyargs \
    -days 3650 -nodes -subj "/CN=${label}" \
    -keyout "$WORK/${label}.key.pem" -out "$WORK/${label}.crt.pem" 2>/dev/null
  softhsm2-util --import "$WORK/${label}.key.pem" \
    --token testtok --label "$label" --id "$id" --pin "$PIN"
  openssl x509 -in "$WORK/${label}.crt.pem" -outform der -out "$WORK/${label}.crt.der"
  pkcs11-tool --module "$MODULE" --pin "$PIN" \
    --write-object "$WORK/${label}.crt.der" --type cert --label "$label" --id "$id"
}

# auth-cert is RSA to mirror production (CSPid: RSA key, TLS 1.3, RSA-PSS).
# sign-cert stays EC as a decoy, proving label selection across key types.
make_identity auth-cert 01 rsa
make_identity sign-cert 02 ec

# Test CA + a server leaf it signs. rustls/webpki rejects a self-signed CA cert
# used as an end-entity (CaUsedAsEndEntity), so the server needs a proper leaf
# (CA:FALSE, serverAuth EKU) chaining to a CA the client trusts (ca.crt.pem).
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -days 3650 -nodes \
  -subj "/CN=test-ca" -keyout "$WORK/ca.key.pem" -out "$WORK/ca.crt.pem" 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
  -subj "/CN=localhost" -keyout "$WORK/server.key.pem" -out "$WORK/server.csr.pem" 2>/dev/null
openssl x509 -req -in "$WORK/server.csr.pem" \
  -CA "$WORK/ca.crt.pem" -CAkey "$WORK/ca.key.pem" -CAcreateserial -days 3650 \
  -extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE") \
  -out "$WORK/server.crt.pem" 2>/dev/null

echo
echo "Objects on token:"
pkcs11-tool --module "$MODULE" -O

cat <<EOF

Test harness ready under: $WORK
  SOFTHSM2_CONF  = $SOFTHSM2_CONF
  PKCS#11 module = $MODULE
  Token PIN      = $PIN
  Client identities: auth-cert (id 01), sign-cert (id 02)
  Server trust   : auth-cert.crt.pem (mTLS client auth)
  Client root    : ca.crt.pem (pass to spike --ca)

Next: ./scripts/mtls-test-server.sh   # https://localhost:8443, requires a client cert
EOF

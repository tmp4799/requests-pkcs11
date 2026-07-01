#!/usr/bin/env python3
"""Read-only CSPid self-test (run on the work machine, CSPid agent running).

Answers the three questions that gate the Rust signer (beads uov / lx7):
  1. What key type is the auth credential (RSA vs EC)?
  2. Does the token expose RSA-PSS mechanisms (decides TLS 1.3 vs forced 1.2)?
  3. Can we SIGN with the private key using only the agent's cached login
     (no manual PIN)? Reading a cert is public; signing needs a logged-in
     session, so this is the real proof for the TLS handshake.

Only enumerates objects and signs a constant throwaway string. Nothing on the
token is modified.

Usage:
  pip install PyKCS11
  PKCS11_MODULE=/opt/cspid/lib/libcspid.so \\
  PKCS11_LABEL=<auth cert label, optional> \\
  [PKCS11_PIN=<pin, only if cache login fails>] \\
  python3 scripts/cspid_selftest.py
"""
import binascii
import os
import sys

import PyKCS11

MODULE = os.environ.get("PKCS11_MODULE", "/opt/cspid/lib/libcspid.so")
LABEL = os.environ.get("PKCS11_LABEL")  # None -> use first private key found
PIN = os.environ.get("PKCS11_PIN")      # None -> rely on agent's cached login


def name(table, value):
    try:
        return table[value]
    except Exception:
        return str(value)


def main():
    lib = PyKCS11.PyKCS11Lib()
    lib.load(MODULE)

    slots = lib.getSlotList(tokenPresent=True)
    if not slots:
        sys.exit("no slot with a token present (is the CSPid agent running?)")
    slot = slots[0]
    print(f"module: {MODULE}")
    print(f"token : {lib.getTokenInfo(slot).label.strip()}")

    # --- (2) mechanisms ---
    mechs = {name(PyKCS11.CKM, m) for m in lib.getMechanismList(slot)}
    relevant = sorted(m for m in mechs if any(k in m for k in ("RSA", "ECDSA", "PSS")))
    print("\nsigning-relevant mechanisms:")
    for m in relevant:
        print(f"  {m}")
    has_pss = any("PSS" in m for m in mechs)
    print(f"\nRSA-PSS present: {has_pss}  "
          f"(no PSS + RSA key => on-token TLS 1.3 impossible, must pin TLS 1.2)")

    session = lib.openSession(slot)

    # --- (1) private keys + key type ---
    tmpl = [(PyKCS11.CKA_CLASS, PyKCS11.CKO_PRIVATE_KEY)]
    if LABEL:
        tmpl.append((PyKCS11.CKA_LABEL, LABEL))
    keys = session.findObjects(tmpl)
    if not keys:
        sys.exit(f"no private key found (LABEL={LABEL!r})")

    print("\nprivate keys:")
    for k in keys:
        lbl, kt, kid = session.getAttributeValue(
            k, [PyKCS11.CKA_LABEL, PyKCS11.CKA_KEY_TYPE, PyKCS11.CKA_ID]
        )
        kid_hex = binascii.hexlify(bytes(kid)).decode() if kid else None
        print(f"  label={lbl!r} type={name(PyKCS11.CKK, kt)} id={kid_hex}")

    key = keys[0]
    key_type = session.getAttributeValue(key, [PyKCS11.CKA_KEY_TYPE])[0]

    # --- (3) test signature, preferring the TLS-relevant mechanism ---
    data = b"requests-pkcs11 self-test"
    if key_type == PyKCS11.CKK_RSA and has_pss:
        mech = PyKCS11.RSA_PSS_Mechanism(
            PyKCS11.CKM_SHA256_RSA_PKCS_PSS,
            PyKCS11.CKM_SHA256, PyKCS11.CKG_MGF1_SHA256, 32,
        )
        mech_label = "CKM_SHA256_RSA_PKCS_PSS"
    elif key_type == PyKCS11.CKK_RSA:
        mech = PyKCS11.Mechanism(PyKCS11.CKM_SHA256_RSA_PKCS)
        mech_label = "CKM_SHA256_RSA_PKCS (v1.5; TLS 1.2 only)"
    else:  # EC
        mech = PyKCS11.Mechanism(PyKCS11.CKM_ECDSA_SHA256)
        mech_label = "CKM_ECDSA_SHA256"

    print(f"\nsigning test ({mech_label})...")
    used_login = False
    try:
        sig = session.sign(key, data, mech)
    except PyKCS11.PyKCS11Error as e:
        if e.value != PyKCS11.CKR_USER_NOT_LOGGED_IN:
            raise
        # Cached login wasn't enough; try an explicit C_Login.
        print("  not logged in via cache; attempting C_Login...")
        session.login(PIN if PIN is not None else "")
        used_login = True
        sig = session.sign(key, data, mech)

    print(f"  OK: {len(bytes(sig))}-byte signature")
    print(f"  explicit C_Login required: {used_login}  "
          f"({'PIN handling needed in app' if used_login else 'agent cache sufficient — no PIN in app'})")


if __name__ == "__main__":
    main()

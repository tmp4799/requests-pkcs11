#!/usr/bin/env python3
"""Query an mTLS API using a PKCS#11 token identity.

Config via env (see SKILL.md): PKCS11_MODULE, PKCS11_LABEL, PKCS11_CA,
and optionally PKCS11_PIN (unset = pre-authenticated token, e.g. CSPid agent).

usage: query.py URL [METHOD [BODY]]
"""
import os
import sys

import requests_pkcs11


def env(name):
    value = os.environ.get(name)
    if not value:
        sys.exit(f"error: {name} is not set (see SKILL.md)")
    return value


def main():
    args = sys.argv[1:]
    if not 1 <= len(args) <= 3:
        sys.exit(__doc__.strip())
    url = args[0]
    method = args[1] if len(args) > 1 else "GET"
    body = args[2] if len(args) > 2 else None
    resp = requests_pkcs11.request(
        module=env("PKCS11_MODULE"),
        label=env("PKCS11_LABEL"),
        url=url,
        ca=env("PKCS11_CA"),
        pin=os.environ.get("PKCS11_PIN"),
        method=method,
        body=body.encode() if body is not None else None,
    )
    print(f"HTTP {resp.status}")
    for name, value in sorted(resp.headers.items()):
        print(f"{name}: {value}")
    print()
    print(resp.text)


if __name__ == "__main__":
    main()

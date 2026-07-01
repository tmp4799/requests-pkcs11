# requests-pkcs11 — Plan

A Python extension (`requests_pkcs11`) that makes HTTPS requests authenticated by an
mTLS client certificate whose private key lives on a CSPID-managed **PKCS#11** token.
The core is written in **Rust** (first real Rust project) and exposed to Python via
**PyO3 / maturin**. A Claude Code skill consumes it to query internal APIs and triage
the results.

Issue tracking lives in **beads** (`bd ready`); IDs are referenced throughout.

---

## Why this can't be done in plain Python `requests`

`requests` → `urllib3` → Python's stdlib `ssl` → OpenSSL. mTLS **client-cert auth**
requires signing the TLS handshake's `CertificateVerify` message with the *private key*.
That key never leaves the PKCS#11 token, so the signature must be produced **on the
token** (a `C_Sign` call). Python's `ssl` module exposes no hook to route the client-auth
signature through a PKCS#11 provider — it only accepts a cert+key loaded from PEM. OpenSSL
*can* do it via its `pkcs11` engine/provider, but `ssl` doesn't let you configure engines.
That is the wall.

Rust's `rustls` **does** expose the necessary hook (a custom client-cert resolver +
`SigningKey`), so the signing can be delegated to the token cleanly and cross-platform.

---

## Confirmed decisions

| Question | Answer |
|---|---|
| Where does the cert/module live, target OS | **Linux** + a PKCS#11 `.so` (CSPID) |
| Python integration | **PyO3 native module**, built with maturin (`import requests_pkcs11`) |
| Auth model | **mTLS client certificate** (token signs the handshake) |
| Build/test environment | **Linux devcontainer** + **SoftHSM2** software token for offline testing |
| Multiple certs on the token | **Likely yes** → we must select the cert by label/ID (confirm at office) |

---

## Architecture

```
requests-pkcs11/                  (cargo workspace)
├── crates/core/        Rust library — the real logic
│   ├── pkcs11          cryptoki: open module, login (PIN), find cert + key by LABEL/ID
│   ├── signer          rustls SigningKey/Signer → delegates C_Sign to the token   ← crux
│   ├── tls             build a rustls ClientConfig with the client-cert resolver
│   ├── client          reqwest::blocking request → status/headers/body(/JSON)
│   └── bin/spike.rs    throwaway CLI to prove the handshake without Python in the loop
├── crates/py/          PyO3 layer (cdylib) — exposes core to Python via maturin
└── python/             packaging + the Claude skill
```

**Rust stack:** `cryptoki` (PKCS#11), `rustls` (≥0.23, custom signer), `reqwest`
(blocking, `rustls-tls`, `default-features=false` — no async, no OpenSSL),
`serde`/`serde_json`, `anyhow`, `clap` (spike only). PyO3 + maturin for the wrapper.

We use `reqwest::blocking` deliberately: no async runtime to learn on top of everything
else, and a synchronous call is the right shape for a request-response tool.

---

## The crux is mostly solved — research outcome

The signer bridge already exists as a crate:
**[`rustls-pkcs11`](https://github.com/mdelete/rustls-pkcs11)** — MIT, v0.1.1 (2026-06),
~410 LOC, built on `cryptoki` 0.12 + `rustls` ≥0.22.

Reading the source (the README undersells it) confirms it:
- advertises **RSA-PSS** (required for TLS 1.3), RSA-PKCS#1, ECDSA P-256/384/521, EdDSA;
- maps rustls `SignatureScheme` → cryptoki `Mechanism`, and converts ECDSA `r‖s` → ASN.1;
- logs in with the PIN at resolver construction; wraps the session in `Arc<Mutex>`.

**Decision: vendor it** into `crates/core` (it's small, MIT, and unproven at 0 stars —
we want control, and the 410 lines are ideal Rust learning material) **and fork it to add
cert/key selection by `CKA_LABEL`/`CKA_ID`.** The upstream uses "first found" discovery,
which will pick the wrong identity on a multi-cert token — which ours likely is.

```rust
// Drop-in shape (upstream); we extend the constructor to take a label/ID.
let tls = rustls::ClientConfig::builder()
    .with_root_certificates(roots)
    .with_client_cert_resolver(Arc::new(
        PKCS11ClientCertResolver::new(Some(pin), "/path/to/cspid.so")? // + label/id
    ));
```

---

## Testing strategy (offline, no CSPID dependency)

Inside the devcontainer, provision **SoftHSM2** as a software PKCS#11 token, generate a
keypair + self-signed client cert in it, and run a local mTLS server (`openssl s_server`
or a tiny Rust server) that requires client auth. We prove the entire path against that.
**Going to production is then just swapping the module path + cert label** to the real
CSPID `.so`. To exercise the label-selection fork, we load **two** certs into the SoftHSM
token and assert the correct one is presented.

---

## Phased plan (with bead IDs)

```
hp3 Devcontainer + SoftHSM2 harness ─┐
                                     ├─► 7um PKCS#11 discovery ─► uov vendor+fork signer (P0)
1m3 Cargo workspace scaffold ────────┘                                  │
                                                                        ▼
                                          4wt rustls config + reqwest e2e GET (P0, spike payoff)
                                                                        │
                                                                        ▼
                                                       l39 PyO3/maturin wrapper
                                                              ├─► 8wj Claude skill + docs
                                                              └─► lx7 hardening + real CSPID module
```

1. **hp3 — Devcontainer + SoftHSM2 harness.** Linux container (Rust, maturin, Python,
   SoftHSM2); provision a test token with a key + self-signed cert; local mTLS test server.
   *Verify:* token lists key+cert via `pkcs11-tool`; server rejects no-cert, accepts cert.
2. **1m3 — Cargo workspace scaffold.** `crates/core` (lib + `bin/spike.rs`), `crates/py`
   (cdylib); add deps; `pyproject.toml`/maturin config. *Verify:* `cargo build` passes.
3. **7um — PKCS#11 session + cert/key discovery.** cryptoki: open module, login, find cert
   + key **by label/ID**. *Verify:* spike prints cert subject + a usable key handle.
4. **uov — Vendor & fork `rustls-pkcs11` (P0).** Vendor the signer; add label/ID selection.
   *Verify:* signs with a label-selected key; signature verifies; correct cert chosen when
   the token holds 2+ certs.
5. **4wt — rustls config + reqwest e2e (P0, spike payoff).** Build the `ClientConfig`, drive
   `reqwest::blocking`. *Verify:* spike gets HTTP 200 from the SoftHSM-backed mTLS server.
6. **l39 — PyO3 + maturin wrapper.** Expose module path, PIN, label, URL, method, headers,
   body → status/headers/body(+JSON). *Verify:* `import requests_pkcs11` works; Python call
   returns 200/JSON against the test server.
7. **8wj — Claude skill + packaging + docs.** Skill that queries an internal API and triages
   results; README for cert/PIN/module setup. *Verify:* skill runs e2e against the test server.
8. **lx7 — Hardening + real CSPID module.** PIN handling (cached/env/prompt), error messages,
   swap to the real `.so` + label. *Verify:* authenticated request against a real internal API.

---

## CSPid (ISC) — what the spike found

CSPid is a **software "virtual smartcard"** from Information Security Corporation exposing a
**standard PKCS#11 API**, so our `rustls` + `cryptoki` + `C_Sign` design is correct and needs
no change. Keys live in AES-256 password-protected PKCS#15 files; FIPS 140-3 L1 (CMVP #4998).

- **Module:** `libcspid.so` on UNIX (default install `/opt/cspid`). Confirm exact path.
- **Algorithms:** RSA up to 8192-bit **and** ECDSA up to 571-bit; SHA-256/384/512. Either key
  type is possible — the real credential's type is still unknown.
- **Login via a running agent — CONFIRMED.** The CSPid agent auto-starts each login session and
  caches the password, so the token is available to apps with no manual PIN (this is what
  Firefox rides on). Verified: cert objects are readable from a Python pkcs11 module with the
  agent running. **Caveat:** reading a cert is a *public* op; signing needs a logged-in session.
  The outstanding proof is a private-key `C_Sign` with no manual PIN — that's what the TLS
  handshake actually needs. Our app likely needs no PIN handling (try without `C_Login` first).
- **DAS mode (transparent to us).** In ISC's role-based/DAS config, `libcspid.so` delegates
  `C_Sign` to a remote DAS server. We still call `Session::sign`; only effects are network
  latency and the key possibly being server-side.
- **App registration.** Apps calling `C_Initialize` are tracked in a "registered applications
  list" (`cspid_ui --register` on Windows). Our app may need registering/authorizing first.
- **Platforms:** officially CentOS/RHEL (prod likely RHEL 8+); the Debian devcontainer is
  irrelevant here — we test against SoftHSM, not `libcspid.so`.

## Confirm at the office (gates `lx7`; may force a TLS-version decision)

These do **not** block building `uov`/`4wt` against SoftHSM, but they decide which code path
runs in production and whether a fallback is required. Ensure the **CSPid agent is running**
and the token unlocked, then:

1. **Key type + labels/IDs** — `pkcs11-tool --module /opt/cspid/lib/libcspid.so --login -O`
   - RSA or EC? key size? Note the `label`/`ID` of each **certificate**, and which is the
     *authentication* cert (clientAuth EKU) vs signing/encryption.
   - Feeds: the `open_identity` label, and the signer's mechanism choice.
2. **Supported mechanisms — the decisive one** — `pkcs11-tool --module .../libcspid.so -M`
   - If the key is **RSA**, look for `RSA-PKCS-PSS` / `SHA256-RSA-PKCS-PSS`.
     **TLS 1.3 requires RSA-PSS for RSA client signatures.** If the token lacks PSS,
     on-token TLS 1.3 client auth is impossible → we must pin **TLS 1.2** (PKCS#1 v1.5,
     `CKM_SHA256_RSA_PKCS`) and confirm the API still accepts 1.2.
   - If **EC**, look for `ECDSA` / `ECDSA-SHA256`; no PSS concern.
3. **Always-authenticate?** — does the key carry `CKA_ALWAYS_AUTHENTICATE`? If so the token
   demands re-auth per signature (context-specific PIN), which changes the signer.
4. **PIN / login flow** — does our app pass a PIN to `C_Login`, or rely on the agent's cached
   (per-session) password? Confirm the agent runs in the target environment.
5. **App registration** — is registration/authorization enforced for new apps, and how on Linux?
6. **Module path** (confirm `/opt/cspid/...`), plus the API's **TLS version** and URL.

**Offline hedge we can do now:** add an **RSA** identity to the SoftHSM harness and prove the
signer's RSA-PSS path before we ever touch the real token. (Note: SoftHSM supporting PSS does
not prove CSPID does — item 2 is still required.)

//! requests-pkcs11 core: mTLS HTTPS requests authenticated by a PKCS#11 token.
//!
//! Scaffold only. Real modules land in later beads:
//!   - 7um: PKCS#11 session + cert/key discovery by label/ID (cryptoki)
//!   - uov: vendored+forked rustls-pkcs11 signer (delegates to C_Sign)
//!   - 4wt: rustls ClientConfig + reqwest::blocking request

pub mod client;
pub mod pkcs11;
pub mod signer;

/// Placeholder so the PyO3 crate still builds until its real API lands (l39).
pub fn placeholder() -> &'static str {
    "requests-pkcs11-core: scaffold"
}

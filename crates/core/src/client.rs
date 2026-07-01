//! Blocking HTTPS client that authenticates with the token-backed identity.
//!
//! Builds a rustls `ClientConfig` whose client-cert resolver signs on the
//! PKCS#11 token, and hands it to `reqwest::blocking`.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::RootCertStore;

use crate::pkcs11::Identity;
use crate::signer::Pkcs11ClientCertResolver;

/// A minimal HTTP response.
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Perform a GET against `url`, presenting `identity` for mTLS. `root_pem` is a
/// PEM bundle of trust anchors for the server (the test server's self-signed
/// cert; in production, the internal CA).
pub fn get(identity: Identity, url: &str, root_pem: &Path) -> Result<Response> {
    let tls = client_config(identity, root_pem)?;
    let client = reqwest::blocking::Client::builder()
        .use_preconfigured_tls(tls)
        .build()?;
    let resp = client.get(url).send()?;
    let status = resp.status().as_u16();
    let body = resp.bytes()?.to_vec();
    Ok(Response { status, body })
}

fn client_config(identity: Identity, root_pem: &Path) -> Result<rustls::ClientConfig> {
    let mut roots = RootCertStore::empty();
    let pem = std::fs::read(root_pem).with_context(|| format!("reading roots {root_pem:?}"))?;
    for cert in rustls_pemfile::certs(&mut pem.as_slice()) {
        roots.add(cert.context("parsing root PEM")?)?;
    }

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_client_cert_resolver(Arc::new(Pkcs11ClientCertResolver::new(identity)));
    Ok(config)
}

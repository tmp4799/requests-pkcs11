//! Spike: locate a PKCS#11 client identity by label and (optionally) perform an
//! mTLS GET with it — the end-to-end proof for beads uov + 4wt.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use requests_pkcs11_core::{client, pkcs11};
use x509_certificate::X509Certificate;

#[derive(Parser)]
#[command(about = "Locate a PKCS#11 client identity and optionally mTLS-GET a URL")]
struct Args {
    /// Path to the PKCS#11 module (.so)
    #[arg(long, env = "PKCS11_MODULE")]
    module: String,
    /// Token user PIN. Omit when the token is pre-authenticated (e.g. CSPid agent).
    #[arg(long, env = "PKCS11_PIN")]
    pin: Option<String>,
    /// CKA_LABEL of the cert + key to select
    #[arg(long, default_value = "auth-cert")]
    label: String,
    /// If set, perform an mTLS GET against this URL
    #[arg(long)]
    url: Option<String>,
    /// PEM bundle of trust anchors for the server (required with --url)
    #[arg(long)]
    ca: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let id = pkcs11::open_identity(&args.module, args.pin.as_deref(), &args.label)?;

    let cert = X509Certificate::from_der(&id.cert_der)?;
    let cn = cert
        .subject_common_name()
        .unwrap_or_else(|| "<no CN>".to_string());
    println!("label={} CN={cn} cert={} bytes", args.label, id.cert_der.len());

    match (args.url, args.ca) {
        (Some(url), Some(ca)) => {
            let resp = client::get(id, &url, &ca)?;
            println!("GET {url} -> HTTP {}", resp.status);
            println!("body: {} bytes", resp.body.len());
        }
        (Some(_), None) => anyhow::bail!("--url requires --ca (server trust anchors)"),
        _ => println!("OK: identity located (no --url, skipping request)"),
    }
    Ok(())
}

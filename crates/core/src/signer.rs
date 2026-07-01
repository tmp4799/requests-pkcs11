//! rustls client-auth signer backed by a PKCS#11 token (RSA-PSS / TLS 1.3).
//!
//! Ported and reduced from rustls-pkcs11 (MIT, github.com/mdelete/rustls-pkcs11):
//! we keep only the RSA-PSS path this environment uses (CSPid: RSA key, TLS 1.3)
//! and wire it onto `open_identity` instead of the crate's own discovery.

use std::sync::{Arc, Mutex};

use cryptoki::mechanism::rsa::{PkcsMgfType, PkcsPssParams};
use cryptoki::mechanism::{Mechanism, MechanismType};
use cryptoki::object::ObjectHandle;
use cryptoki::session::Session;
use rustls::client::ResolvesClientCert;
use rustls::pki_types::CertificateDer;
use rustls::sign::{CertifiedKey, Signer, SigningKey};
use rustls::{Error, SignatureAlgorithm, SignatureScheme};

use crate::pkcs11::Identity;

/// A logged-in PKCS#11 session shared with the TLS stack. rustls may sign from
/// the handshake thread, so access is serialised behind a mutex.
type SharedSession = Arc<Mutex<Session>>;

/// RSA-PSS schemes we support, in preference order (all TLS 1.3).
const SCHEMES: &[SignatureScheme] = &[
    SignatureScheme::RSA_PSS_SHA256,
    SignatureScheme::RSA_PSS_SHA384,
    SignatureScheme::RSA_PSS_SHA512,
];

#[derive(Debug)]
struct Pkcs11SigningKey {
    session: SharedSession,
    key: ObjectHandle,
}

impl SigningKey for Pkcs11SigningKey {
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>> {
        let scheme = *SCHEMES.iter().find(|s| offered.contains(s))?;
        Some(Box::new(Pkcs11Signer {
            session: self.session.clone(),
            key: self.key,
            scheme,
        }))
    }

    fn algorithm(&self) -> SignatureAlgorithm {
        SignatureAlgorithm::RSA
    }
}

#[derive(Debug)]
struct Pkcs11Signer {
    session: SharedSession,
    key: ObjectHandle,
    scheme: SignatureScheme,
}

impl Signer for Pkcs11Signer {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Error> {
        // CKM_*_RSA_PKCS_PSS hashes on the token, so `message` is passed
        // unhashed, as rustls requires.
        let mechanism = pss_mechanism(self.scheme);
        let session = self
            .session
            .lock()
            .map_err(|_| Error::General("PKCS#11 session mutex poisoned".into()))?;
        session
            .sign(&mechanism, self.key, message)
            .map_err(|e| Error::General(format!("PKCS#11 C_Sign failed: {e}")))
    }

    fn scheme(&self) -> SignatureScheme {
        self.scheme
    }
}

/// Map a rustls RSA-PSS scheme to the matching cryptoki mechanism + params.
fn pss_mechanism(scheme: SignatureScheme) -> Mechanism<'static> {
    let (hash, mgf, salt) = match scheme {
        SignatureScheme::RSA_PSS_SHA384 => (MechanismType::SHA384, PkcsMgfType::MGF1_SHA384, 48u64),
        SignatureScheme::RSA_PSS_SHA512 => (MechanismType::SHA512, PkcsMgfType::MGF1_SHA512, 64u64),
        _ => (MechanismType::SHA256, PkcsMgfType::MGF1_SHA256, 32u64),
    };
    let params = PkcsPssParams {
        hash_alg: hash,
        mgf,
        s_len: salt.into(),
    };
    match scheme {
        SignatureScheme::RSA_PSS_SHA384 => Mechanism::Sha384RsaPkcsPss(params),
        SignatureScheme::RSA_PSS_SHA512 => Mechanism::Sha512RsaPkcsPss(params),
        _ => Mechanism::Sha256RsaPkcsPss(params),
    }
}

/// Resolves the client certificate to our token-backed identity for every
/// certificate request (we present a single fixed identity).
#[derive(Debug)]
pub struct Pkcs11ClientCertResolver {
    certified_key: Arc<CertifiedKey>,
}

impl Pkcs11ClientCertResolver {
    /// Build a resolver from a located token identity (see `open_identity`).
    pub fn new(identity: Identity) -> Self {
        let cert = CertificateDer::from(identity.cert_der);
        let session = Arc::new(Mutex::new(identity.session));
        let signing_key: Arc<dyn SigningKey> = Arc::new(Pkcs11SigningKey {
            session,
            key: identity.private_key,
        });
        let certified_key = Arc::new(CertifiedKey::new(vec![cert], signing_key));
        Self { certified_key }
    }
}

impl ResolvesClientCert for Pkcs11ClientCertResolver {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[SignatureScheme],
    ) -> Option<Arc<CertifiedKey>> {
        Some(self.certified_key.clone())
    }

    fn has_certs(&self) -> bool {
        true
    }
}

//! Locate a client identity (cert + private key) on a PKCS#11 token by label.
//!
//! The real CSPID token is expected to hold several certs, so we always select
//! by CKA_LABEL rather than "first found".

use anyhow::{anyhow, Context, Result};
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::object::{Attribute, AttributeType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;

/// A client identity located on a token: a logged-in session, the DER-encoded
/// certificate, and the private-key handle to sign with. The session owns the
/// login state and must outlive any use of `private_key`.
pub struct Identity {
    pub session: Session,
    pub cert_der: Vec<u8>,
    pub private_key: ObjectHandle,
}

/// Load `module_path`, open the first token, optionally log in, and return the
/// certificate + private key whose `CKA_LABEL` equals `label`.
///
/// `pin` is `None` when the token is already authenticated out-of-band (e.g. the
/// CSPid agent's cached login); pass `Some` for tokens that require `C_Login`
/// (e.g. SoftHSM in tests).
pub fn open_identity(module_path: &str, pin: Option<&str>, label: &str) -> Result<Identity> {
    let pkcs11 = Pkcs11::new(module_path)
        .with_context(|| format!("loading PKCS#11 module {module_path}"))?;
    pkcs11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))?;

    let slot = pkcs11
        .get_slots_with_token()?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no slot with a token present"))?;

    let session = pkcs11.open_ro_session(slot)?;
    if let Some(pin) = pin {
        session.login(UserType::User, Some(&AuthPin::new(pin.into())))?;
    }

    let label_bytes = label.as_bytes().to_vec();

    let cert = find_one(&session, ObjectClass::CERTIFICATE, &label_bytes)
        .with_context(|| format!("certificate labelled {label:?}"))?;
    let cert_der = match session
        .get_attributes(cert, &[AttributeType::Value])?
        .into_iter()
        .next()
    {
        Some(Attribute::Value(v)) => v,
        _ => return Err(anyhow!("certificate {label:?} has no CKA_VALUE")),
    };

    let private_key = find_one(&session, ObjectClass::PRIVATE_KEY, &label_bytes)
        .with_context(|| format!("private key labelled {label:?}"))?;

    Ok(Identity {
        session,
        cert_der,
        private_key,
    })
}

/// Find exactly one object of `class` with the given label; error otherwise.
fn find_one(session: &Session, class: ObjectClass, label: &[u8]) -> Result<ObjectHandle> {
    let mut found =
        session.find_objects(&[Attribute::Class(class), Attribute::Label(label.to_vec())])?;
    match found.len() {
        0 => Err(anyhow!("not found")),
        1 => Ok(found.remove(0)),
        n => Err(anyhow!("expected exactly one, found {n}")),
    }
}

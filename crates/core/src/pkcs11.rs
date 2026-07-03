//! Locate a client identity (cert + private key) on a PKCS#11 token by label.
//!
//! The real CSPID token is expected to hold several certs, so we always select
//! by CKA_LABEL rather than "first found".

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, Context, Result};
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::error::RvError;
use cryptoki::object::{Attribute, AttributeType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;

/// Initialized PKCS#11 contexts, keyed by module path. C_Initialize is
/// process-global per module, so per-call init breaks concurrent callers:
/// overlapping C_Initialize fails, and dropping the last handle dlcloses a
/// still-initialized module under other threads' live sessions. Each module is
/// loaded and initialized once; the map holds a clone for the life of the
/// process so it is never unloaded (standard for PKCS#11 in a library).
static CONTEXTS: LazyLock<Mutex<HashMap<String, Pkcs11>>> = LazyLock::new(Default::default);

/// Return the shared initialized context for `module_path`, loading and
/// initializing it on first use.
fn shared_context(module_path: &str) -> Result<Pkcs11> {
    let mut contexts = CONTEXTS.lock().expect("PKCS#11 context map poisoned");
    if let Some(ctx) = contexts.get(module_path) {
        return Ok(ctx.clone());
    }
    let pkcs11 = Pkcs11::new(module_path)
        .with_context(|| format!("loading PKCS#11 module {module_path}"))?;
    pkcs11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))?;
    contexts.insert(module_path.to_string(), pkcs11.clone());
    Ok(pkcs11)
}

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
    let pkcs11 = shared_context(module_path)?;

    let slot = pkcs11
        .get_slots_with_token()?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no slot with a token present"))?;

    let session = pkcs11.open_ro_session(slot)?;
    if let Some(pin) = pin {
        // Login state is per-token per-process, so on a reused context an
        // earlier call may have already logged in.
        match session.login(UserType::User, Some(&AuthPin::new(pin.into()))) {
            Ok(()) | Err(cryptoki::error::Error::Pkcs11(RvError::UserAlreadyLoggedIn, _)) => {}
            Err(e) => return Err(e.into()),
        }
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

//! PyO3 wrapper exposing the core as the `requests_pkcs11` Python module.

use std::collections::HashMap;
use std::path::PathBuf;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use requests_pkcs11_core::{client, pkcs11};

/// An HTTP response.
#[pyclass]
struct Response {
    #[pyo3(get)]
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[pymethods]
impl Response {
    /// Response headers as a dict (last value wins on duplicate names).
    #[getter]
    fn headers(&self) -> HashMap<String, String> {
        self.headers.iter().cloned().collect()
    }

    /// Raw response body.
    #[getter]
    fn body<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.body)
    }

    /// Body decoded as UTF-8 (lossy).
    #[getter]
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Body parsed as JSON.
    fn json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("json")?
            .call_method1("loads", (PyBytes::new(py, &self.body),))
    }

    fn __repr__(&self) -> String {
        format!("<Response [{}]>", self.status)
    }
}

/// Perform an HTTPS request authenticated by the client cert + private key
/// labelled `label` on the PKCS#11 token loaded from `module`.
///
/// `ca` is a PEM bundle of trust anchors for the server. `pin` is None when
/// the token is already authenticated out-of-band (e.g. the CSPid agent);
/// `body` is bytes.
#[pyfunction]
#[pyo3(signature = (module, label, url, ca, pin=None, method="GET", headers=None, body=None))]
#[allow(clippy::too_many_arguments)]
fn request(
    py: Python<'_>,
    module: &str,
    label: &str,
    url: &str,
    ca: PathBuf,
    pin: Option<&str>,
    method: &str,
    headers: Option<HashMap<String, String>>,
    body: Option<Vec<u8>>,
) -> PyResult<Response> {
    let headers: Vec<(String, String)> = headers.unwrap_or_default().into_iter().collect();
    let resp = py
        .detach(|| -> anyhow::Result<client::Response> {
            let identity = pkcs11::open_identity(module, pin, label)?;
            client::request(identity, method, url, &headers, body, &ca)
        })
        .map_err(|e| PyRuntimeError::new_err(format!("{e:#}")))?;
    Ok(Response {
        status: resp.status,
        headers: resp.headers,
        body: resp.body,
    })
}

#[pymodule]
fn requests_pkcs11(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Response>()?;
    m.add_function(wrap_pyfunction!(request, m)?)?;
    Ok(())
}

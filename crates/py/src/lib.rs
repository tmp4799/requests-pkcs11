//! PyO3 wrapper exposing the core as the `requests_pkcs11` Python module.
//! Scaffold only; the real request API lands in bead l39.

use pyo3::prelude::*;

/// Temporary smoke-test function so `import requests_pkcs11` works end-to-end.
#[pyfunction]
fn placeholder() -> PyResult<String> {
    Ok(requests_pkcs11_core::placeholder().to_string())
}

#[pymodule]
fn requests_pkcs11(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(placeholder, m)?)?;
    Ok(())
}

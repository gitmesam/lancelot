use anyhow::Error;
use lancelot::{
    loader::pe::{PEError, PE as lPE},
    module::ModuleError,
    pagemap::PageMapError,
    util::UtilError,
};
use pyo3::{self, prelude::*, types::PyBytes, wrap_pyfunction};

/// ValueError -> "you're doing something wrong"
fn to_value_error(e: anyhow::Error) -> PyErr {
    pyo3::exceptions::ValueError::py_err(format!("{}", e))
}

fn to_py_err(e: Error) -> PyErr {
    match e.downcast_ref::<PEError>() {
        Some(PEError::FormatNotSupported(_)) => return to_value_error(e),
        Some(PEError::MalformedPEFile(_)) => return to_value_error(e),
        None => (),
    };

    #[allow(clippy::single_match)]
    match e.downcast_ref::<ModuleError>() {
        Some(ModuleError::InvalidAddress(_)) => return to_value_error(e),
        None => (),
    };

    match e.downcast_ref::<UtilError>() {
        Some(UtilError::FileAccess) => return to_value_error(e),
        Some(UtilError::FileFormat) => return to_value_error(e),
        None => (),
    };

    #[allow(clippy::single_match)]
    match e.downcast_ref::<PageMapError>() {
        Some(PageMapError::NotMapped) => return to_value_error(e),
        None => (),
    };

    to_value_error(e)
}

#[pyfunction]
fn from_bytes(buf: &PyBytes) -> PyResult<PE> {
    Ok(PE {
        inner: lPE::from_bytes(buf.as_bytes()).map_err(to_py_err)?,
    })
}

#[pyclass]
struct PE {
    inner: lPE,
}

#[pymethods]
impl PE {
    #[getter]
    fn arch(&self) -> PyResult<&'static str> {
        Ok(match self.inner.module.arch {
            lancelot::module::Arch::X32 => "x32",
            lancelot::module::Arch::X64 => "x64",
        })
    }
}

#[pymodule]
fn lancelot(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PE>()?;
    m.add_wrapped(wrap_pyfunction!(from_bytes))?;

    Ok(())
}

#![allow(dead_code, unused_variables, unused_mut)]

pub mod canvas;
pub mod graph;
pub mod layout;
pub mod mermaid;
pub mod parse;
pub mod sequence;

#[cfg(feature = "python")]
mod py_bindings {
    use pyo3::prelude::*;

    #[pyfunction]
    fn render(src: &str) -> Option<String> {
        crate::mermaid::render(src)
    }

    #[pymodule]
    fn termermaid(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add_function(wrap_pyfunction!(render, m)?)?;
        Ok(())
    }
}

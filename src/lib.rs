//! FlareML's native model-checking pipeline.
pub mod checker;
pub mod diagnostics;
pub mod graph;
pub mod model;
pub mod semantics;
pub mod syntax;
pub mod temporal;
pub mod trace;

pub fn compile(source: &str, check: Option<&str>) -> syntax::Result<model::Program> {
    model::Program::build(syntax::parse(source)?, check)
}

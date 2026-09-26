//! FlareML's native model-checking pipeline.
pub mod checker;
pub mod choices;
mod claims;
pub mod diagnostics;
mod evaluation;
mod functions;
pub mod graph;
mod messaging;
pub mod model;
mod observations;
pub mod semantics;
pub mod spawning;
pub mod syntax;
pub mod temporal;
pub mod trace;

pub fn compile(source: &str, check: Option<&str>) -> syntax::Result<model::Program> {
    model::Program::build(syntax::parse(source)?, check)
}

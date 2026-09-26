//! Host safety, not modeled state. Per-thread guards reset for each outermost
//! evaluation, including after errors. Bound both nesting and combinatorial work.
use crate::syntax::{Error, Result, Span};
use std::cell::Cell;
thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static STEPS: Cell<usize> = const { Cell::new(0) };
}
pub(crate) struct EvaluationGuard;
impl EvaluationGuard {
    pub(crate) fn enter(span: Span) -> Result<Self> {
        DEPTH.with(|depth| {
            if depth.get() >= 128 {
                return Err(Error::new(span, "LIMIT: evaluation nesting exceeds 128"));
            }
            STEPS.with(|steps| {
                if depth.get() == 0 {
                    steps.set(0);
                }
                if steps.get() >= 100_000 {
                    return Err(Error::new(
                        span,
                        "LIMIT: local evaluation exceeds 100000 steps",
                    ));
                }
                steps.set(steps.get() + 1);
                depth.set(depth.get() + 1);
                Ok(Self)
            })
        })
    }
}
impl Drop for EvaluationGuard {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

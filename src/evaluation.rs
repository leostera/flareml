//! Host-stack safety guard, not modeled state. The guard is thread-local so
//! parallel checks cannot consume each other's budget, and unwinding releases it.
use crate::syntax::{Error, Result, Span};
use std::cell::Cell;
thread_local! { static DEPTH: Cell<usize> = const { Cell::new(0) }; }

pub(crate) struct EvaluationGuard;
impl EvaluationGuard {
    pub(crate) fn enter(span: Span) -> Result<Self> {
        DEPTH.with(|depth| {
            if depth.get() >= 128 {
                Err(Error::new(span, "LIMIT: evaluation nesting exceeds 128"))
            } else {
                depth.set(depth.get() + 1);
                Ok(Self)
            }
        })
    }
}
impl Drop for EvaluationGuard {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

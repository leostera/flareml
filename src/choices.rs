//! Bounded prefix enumeration using the SAME local interpreter as deterministic
//! turns and replay. Prefixes are re-executed, never partially published.
use crate::{
    graph::Budget,
    model::Program,
    semantics::{Env, Outbox, State, Value},
    syntax::{Error, Expr, Result, Span, Stmt},
};
use serde::{Deserialize, Serialize};

pub const MAX_CHOICE_ENCOUNTERS: usize = 128;
pub const MAX_TURN_ATTEMPTS: usize = 4096;
const NEED_CHOICE: &str = "internal: choice prefix needs extension";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub encounter: usize,
    pub span: Span,
    pub calls: Vec<Span>,
    pub candidate: usize,
    pub value: Value,
}

pub(crate) struct Turn<'a> {
    pub outbox: Outbox,
    pub choices: Vec<Choice>,
    pub calls: Vec<Span>,
    pub budget: Option<&'a Budget>,
    tape: Vec<Choice>,
    pending: Vec<Choice>,
    replay: bool,
}
impl Turn<'_> {
    pub fn poll(&self) -> Result<()> {
        if let Some(budget) = self.budget {
            budget.poll()?;
        }
        Ok(())
    }
    pub fn choose(
        &mut self,
        p: &Program,
        candidates: &[Expr],
        span: Span,
        env: &Env,
        state: &State,
    ) -> Result<Value> {
        self.poll()?;
        let encounter = self.choices.len();
        if encounter >= MAX_CHOICE_ENCOUNTERS {
            return Err(Error::new(
                span,
                "LIMIT: turn exceeds 128 choice encounters",
            ));
        }
        if candidates.len() > MAX_TURN_ATTEMPTS {
            return Err(Error::new(
                span,
                "LIMIT: choice expansion exceeds 4096 candidates",
            ));
        }
        let mut values = Vec::new();
        for expr in candidates {
            self.poll()?;
            let value = p.eval(expr, env, state)?;
            p.check_value(&value, expr.span)?;
            values.push(value);
        }
        if let Some(selection) = self.tape.get(encounter) {
            if selection.encounter != encounter
                || selection.span != span
                || selection.calls != self.calls
                || values.get(selection.candidate) != Some(&selection.value)
            {
                return Err(Error::new(
                    span,
                    "invalid trace: choice transcript mismatch",
                ));
            }
            self.choices.push(selection.clone());
            return Ok(selection.value.clone());
        }
        if self.replay {
            return Err(Error::new(span, "invalid trace: missing choice selection"));
        }
        self.pending = values
            .into_iter()
            .enumerate()
            .map(|(candidate, value)| Choice {
                encounter,
                span,
                calls: self.calls.clone(),
                candidate,
                value,
            })
            .collect();
        Err(Error::new(span, NEED_CHOICE))
    }
}

impl Program {
    pub(crate) fn turn_outcomes<'a>(
        &self,
        body: &[Stmt],
        env: &Env,
        state: &State,
        budget: Option<&'a Budget>,
        tape: Option<&[Choice]>,
    ) -> Result<Vec<(Value, Turn<'a>)>> {
        if tape.is_some_and(|choices| choices.len() > MAX_CHOICE_ENCOUNTERS) {
            return Err(Error::new(
                Span::default(),
                "invalid trace: too many choice selections",
            ));
        }
        // Keep a guard alive over ALL prefix executions: re-execution does not
        // reset the shared 100,000-entry evaluation budget between alternatives.
        let _guard = crate::evaluation::EvaluationGuard::enter(Span::default())?;
        let mut prefixes = vec![tape.unwrap_or_default().to_vec()];
        let mut outcomes = Vec::new();
        let mut attempts = 0;
        while let Some(prefix) = prefixes.pop() {
            attempts += 1;
            if attempts > MAX_TURN_ATTEMPTS {
                return Err(Error::new(
                    Span::default(),
                    "LIMIT: turn exceeds 4096 prefix executions",
                ));
            }
            let mut turn = Turn {
                outbox: Vec::new(),
                choices: Vec::new(),
                calls: Vec::new(),
                budget,
                tape: prefix,
                pending: Vec::new(),
                replay: tape.is_some(),
            };
            turn.poll()?;
            match self.eval_body(body, &mut env.clone(), state, Some(&mut turn)) {
                Ok(value) => {
                    if turn.choices.len() != turn.tape.len() {
                        return Err(Error::new(
                            Span::default(),
                            "invalid trace: unused choice selections",
                        ));
                    }
                    turn.tape.clear();
                    outcomes.push((value, turn));
                }
                Err(e) if e.message == NEED_CHOICE && tape.is_none() => {
                    if attempts + prefixes.len() + turn.pending.len() > MAX_TURN_ATTEMPTS {
                        return Err(Error::new(
                            e.span,
                            "LIMIT: turn exceeds 4096 prefix executions",
                        ));
                    }
                    for selection in turn.pending.into_iter().rev() {
                        let mut prefix = turn.choices.clone();
                        prefix.push(selection);
                        prefixes.push(prefix);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_enabledness_matches_exhaustive_successors() {
        let source = "actor A { handle_message(m: unit): unit { let n = choose([true, false]); send(A, ()); } } actor B { handle_message(m: unit): unit { () } } property \"p\" { always true } check C { mailbox_bound = 1 inputs { once send(A, ()) once send(B, ()) } fairness { weak runtime.progress } }";
        let p = crate::compile(source, None).unwrap();
        let initial = p.initial().unwrap();
        let mut seen = std::collections::HashSet::from([initial.clone()]);
        let mut pending = vec![initial];
        while let Some(state) = pending.pop() {
            let steps = p.successors(&state).unwrap();
            let expected = steps
                .iter()
                .filter(|s| s.action.fair)
                .map(|s| s.action.id.clone())
                .collect();
            assert_eq!(p.fair_enabled(&state), expected);
            for step in steps {
                if seen.insert(step.state.clone()) {
                    pending.push(step.state);
                }
            }
        }
        assert!(seen.len() > 1);
    }
    #[test]
    fn expansion_obeys_the_checkers_deadline() {
        let source = "actor A { handle_message(m: unit): unit { let n = choose([true, false]); () } } property \"p\" { always true } check C { mailbox_bound = 1 inputs { once send(A, ()) } }";
        let p = crate::compile(source, None).unwrap();
        let submitted = p.successors(&p.initial().unwrap()).unwrap().remove(1).state;
        let budget = Budget::new(std::time::Duration::ZERO);
        let error = p
            .message_successors(&submitted, Some(&budget), None)
            .unwrap_err();
        assert_eq!(error.message, "LIMIT: timeout");
        assert!(
            p.successors(&submitted).is_ok(),
            "guards reset after failure"
        );
    }
}

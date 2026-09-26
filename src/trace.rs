//! Versioned artifacts are re-executed, not trusted as serialized assertions.
use crate::{
    graph::{Graph, Walk},
    model::Program,
    semantics::{Action, Env, State},
    syntax::*,
    temporal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Only this artifact layout/meaning is supported; older traces must be regenerated.
pub const FORMAT_VERSION: u32 = 8;

pub fn source_hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub format_version: u32,
    pub tool_version: String,
    pub source_hash: String,
    pub check: String,
    pub claim: String,
    pub kind: ClaimKind,
    pub clause: Option<usize>,
    pub loop_start: Option<usize>,
    pub weak_progress: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox_bound: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_bound: Option<usize>,
    pub spawn_bounds: std::collections::BTreeMap<String, usize>,
    pub states: Vec<State>,
    pub actions: Vec<Action>,
}
impl Trace {
    #[allow(clippy::too_many_arguments)]
    pub fn from_walk(
        source: &str,
        p: &Program,
        claim: &Claim,
        clause: Option<usize>,
        walk: &Walk,
        g: &Graph,
        states: &[State],
        actions: &[Vec<Action>],
    ) -> Self {
        let mut trace_states = vec![states[walk.start].clone()];
        let mut chosen = vec![];
        for &(s, e) in &walk.steps {
            chosen.push(actions[s][e].clone());
            trace_states.push(states[g.edges[s][e].to].clone());
        }
        Self {
            format_version: FORMAT_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").into(),
            source_hash: source_hash(source),
            check: p.check.name.clone(),
            claim: claim.name.clone(),
            kind: claim.kind.clone(),
            clause,
            loop_start: walk.loop_start,
            weak_progress: p.check.fair,
            mailbox_bound: p.check.mailbox_bound,
            message_bound: p.check.message_bound,
            spawn_bounds: p.check.spawn_bounds.clone(),
            states: trace_states,
            actions: chosen,
        }
    }
    pub fn validate(&self, source: &str, p: &Program) -> Result<()> {
        let bad = |s: &str| Error::new(Span::default(), format!("invalid trace: {s}"));
        if self.format_version != FORMAT_VERSION || self.tool_version != env!("CARGO_PKG_VERSION") {
            return Err(bad("unsupported format/tool version"));
        }
        if self.source_hash != source_hash(source)
            || self.check != p.check.name
            || self.weak_progress != p.check.fair
            || self.mailbox_bound != p.check.mailbox_bound
            || self.message_bound != p.check.message_bound
            || self.spawn_bounds != p.check.spawn_bounds
        {
            return Err(bad("model, check, bounds, or fairness mismatch"));
        }
        if self.states.is_empty()
            || self.states.len() != self.actions.len() + 1
            || self.actions.len() > 100_000
        {
            return Err(bad("invalid state/action count"));
        }
        let mut current = p.initial()?;
        if current != self.states[0] {
            return Err(bad("initial state mismatch"));
        }
        for (i, action) in self.actions.iter().enumerate() {
            let step = p
                .message_successors(&current, None, Some(action))?
                .into_iter()
                .find(|s| &s.action == action)
                .ok_or_else(|| bad("action is not enabled or metadata was changed"))?;
            if step.state != self.states[i + 1] {
                return Err(bad("successor state mismatch"));
            }
            current = step.state;
        }
        if let Some(start) = self.loop_start {
            if start >= self.actions.len() || self.states[start] != current {
                return Err(bad("loop does not close"));
            }
            let mut fair_actions = BTreeSet::new();
            let mut enabled = vec![];
            for s in &self.states[start..self.actions.len()] {
                let set = p.fair_enabled(s);
                fair_actions.extend(set.iter().cloned());
                enabled.push(set);
            }
            for action in fair_actions {
                if enabled.iter().all(|set| set.contains(&action))
                    && !self.actions[start..]
                        .iter()
                        .any(|a| a.fair && a.id == action)
                {
                    return Err(bad("loop violates declared weak fairness"));
                }
            }
        }
        let claim = p
            .model
            .claims
            .iter()
            .find(|c| c.name == self.claim && c.kind == self.kind)
            .ok_or_else(|| bad("unknown claim"))?;
        match claim.kind {
            ClaimKind::Invariant => {
                if self.loop_start.is_some() || self.clause.is_some() {
                    return Err(bad("unexpected invariant loop/clause"));
                }
                if p.predicate(&claim.body, &Env::new(), &current)? {
                    return Err(bad("invariant is not violated at the end"));
                }
            }
            ClaimKind::Cover => {
                if self.loop_start.is_some() || self.clause.is_some() {
                    return Err(bad("unexpected cover loop/clause"));
                }
                if !p.predicate(&claim.body, &Env::new(), &current)? {
                    return Err(bad("cover is not reached"));
                }
            }
            ClaimKind::Property => {
                let cs = temporal::clauses(p, &self.states[0], &claim.body)?;
                let c = self
                    .clause
                    .and_then(|i| cs.get(i))
                    .ok_or_else(|| bad("invalid temporal clause"))?;
                if self.loop_start.is_none() {
                    match c.kind {
                        temporal::Kind::Always | temporal::Kind::Persistence => {}
                        temporal::Kind::Until => {
                            let mut failed = false;
                            for s in &self.states {
                                if p.predicate(c.q.as_ref().expect("until Q"), &c.env, s)? {
                                    break;
                                }
                                if !p.predicate(&c.p, &c.env, s)? {
                                    failed = true;
                                    break;
                                }
                            }
                            if !failed {
                                return Err(bad(
                                    "until needs a finite bad prefix or a closed loop",
                                ));
                            }
                        }
                        _ => return Err(bad("liveness counterexample requires a loop")),
                    }
                }
                let states = if self.loop_start.is_some() {
                    &self.states[..self.states.len() - 1]
                } else {
                    &self.states
                };
                if temporal::on_trace(p, states, self.loop_start, &c.expr, &c.env)? {
                    return Err(bad("temporal property holds on this trace"));
                }
            }
        }
        Ok(())
    }
}

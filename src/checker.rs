//! Breadth-first exploration with exact state equality and truthful cutoff reporting.
use crate::{
    graph::{self, Budget, Edge, Graph, Walk},
    model::Program,
    semantics::{Action, Env},
    syntax::*,
    temporal,
    trace::Trace,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Debug)]
pub struct Options {
    pub max_states: usize,
    pub max_depth: usize,
    pub timeout: Duration,
    pub property: Option<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            max_states: 100_000,
            max_depth: 1000,
            timeout: Duration::from_secs(30),
            property: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    VerifiedInScope,
    Violated,
    Inconclusive,
}
impl Status {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::VerifiedInScope => 0,
            Self::Violated => 1,
            Self::Inconclusive => 3,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimResult {
    pub name: String,
    pub kind: ClaimKind,
    pub result: String,
    pub span: Span,
    pub note: Option<String>,
    pub witness: Option<Trace>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub status: Status,
    pub check: String,
    pub semantics: String,
    pub weak_progress: bool,
    pub assumptions: Vec<String>,
    pub states: usize,
    pub edges: usize,
    pub complete: bool,
    pub cutoff: Option<String>,
    pub claims: Vec<ClaimResult>,
    pub max_states: usize,
    pub max_depth: usize,
    pub timeout_ms: u128,
    pub input_slots: usize,
    pub not_checked: Vec<String>,
}
impl Report {
    pub fn witness(&self) -> Option<&Trace> {
        self.claims
            .iter()
            .find(|c| c.result == "VIOLATED")
            .and_then(|c| c.witness.as_ref())
            .or_else(|| self.claims.iter().find_map(|c| c.witness.as_ref()))
    }
}
pub fn check(source: &str, p: &Program, options: &Options) -> Result<Report> {
    if options.max_states == 0 {
        return Err(Error::new(Span::default(), "max-states must be positive"));
    }
    if let Some(n) = &options.property
        && !p.model.claims.iter().any(|c| &c.name == n)
    {
        return Err(Error::new(Span::default(), "unknown selected property"));
    }
    let budget = Budget::new(options.timeout);
    let initial = p.initial()?;
    let claims: Vec<_> = p
        .model
        .claims
        .iter()
        .filter(|c| options.property.as_ref().is_none_or(|n| n == &c.name))
        .collect();
    let mut report = Report {
        status: Status::Inconclusive,
        check: p.check.name.clone(),
        semantics: p.check.semantics.clone(),
        weak_progress: p.check.fair,
        assumptions: vec![
            if p.check.semantics == "actors-v2" {
                "finite one-shot external send workload; optional inputs may remain unsubmitted"
                    .into()
            } else {
                "finite one-shot input workload; unaccepted inputs may remain unaccepted".into()
            },
            if p.tables.is_empty() {
                "no D1 tables selected; external resource failures, crashes, and unknown commit outcomes are not modeled".into()
            } else {
                "D1 primary-only; no replication, transport failures, crashes, or unknown commit outcomes".into()
            },
            if p.check.semantics == "actors-v2" {
                format!(
                    "actors-v2: finite FIFO mailboxes (capacity {} per address), run-to-completion state-in/state-out turns; outgoing sends become visible atomically with state commit; retained state is not durable",
                    p.check.mailbox_bound.expect("validated profile")
                )
            } else if p.check.semantics == "actors-v1" {
                "actors-v1: finite keyed state and direct typed request/reply; callers suspend across calls; local steps run to the next external effect; state retention is not durability".into()
            } else {
                "actors-v0: stateless and one-instance stateful actors; local steps run to the next external effect; state retention is not durability".into()
            },
            if p.check.semantics == "actors-v2" {
                "internal mailbox processing is fault-free when weak progress is declared; optional inputs are not forced; loss, duplication, transport failures, timeouts, restarts, queues, and persistence are not modeled".into()
            } else if p.check.semantics == "actors-v1" {
                "actor calls assume eventual fault-free delivery when weak progress is declared; transport failures, timeouts, restarts, queues and persistence beyond D1 are not modeled".into()
            } else {
                "actor-to-actor calls, per-key instances, restarts, queues, and persistence beyond D1 are not modeled".into()
            },
            if let Some(bound) = p.check.message_bound {
                format!(
                    "message observations: {bound} lifetime slots per actor declaration, including external sends; no slot reuse; exhaustion is inconclusive"
                )
            } else {
                "no lifetime message history selected; external input observations remain finite"
                    .into()
            },
            "exact state equality; no symmetry or partial-order reduction".into(),
        ],
        states: 0,
        edges: 0,
        complete: false,
        cutoff: None,
        claims: claims
            .iter()
            .map(|c| ClaimResult {
                name: c.name.clone(),
                kind: c.kind.clone(),
                result: "INCONCLUSIVE".into(),
                span: c.span,
                note: None,
                witness: None,
            })
            .collect(),
        max_states: options.max_states,
        max_depth: options.max_depth,
        timeout_ms: options.timeout.as_millis(),
        input_slots: p.check.inputs.len(),
        not_checked: p
            .model
            .claims
            .iter()
            .filter(|c| options.property.as_ref().is_some_and(|n| n != &c.name))
            .map(|c| c.name.clone())
            .collect(),
    };
    let mut states = vec![initial.clone()];
    let mut intern = HashMap::from([(initial, 0usize)]);
    let mut graph = Graph {
        edges: vec![vec![]],
    };
    let mut actions: Vec<Vec<Action>> = vec![vec![]];
    let mut prev = vec![None];
    let mut depths = vec![0usize];
    let mut cursor = 0;
    let mut failure = false;
    'search: while cursor < states.len() {
        if let Err(e) = budget.poll() {
            report.cutoff = Some(e.message);
            break;
        }
        for (i, c) in claims.iter().enumerate() {
            if c.kind == ClaimKind::Property {
                continue;
            }
            let value = p.predicate(&c.body, &Env::new(), &states[cursor])?;
            if c.kind == ClaimKind::Invariant && !value {
                let walk = graph::backtrack(&graph, &prev, cursor);
                let trace = Trace::from_walk(source, p, c, None, &walk, &graph, &states, &actions);
                trace.validate(source, p)?;
                report.claims[i].result = "VIOLATED".into();
                report.claims[i].witness = Some(trace);
                failure = true;
                break 'search;
            }
            if c.kind == ClaimKind::Cover && value && report.claims[i].witness.is_none() {
                let walk = graph::backtrack(&graph, &prev, cursor);
                let trace = Trace::from_walk(source, p, c, None, &walk, &graph, &states, &actions);
                trace.validate(source, p)?;
                report.claims[i].result = "REACHED".into();
                report.claims[i].witness = Some(trace);
            }
        }
        let successors = match p.successors(&states[cursor]) {
            Ok(s) => s,
            Err(e) if e.message.starts_with("LIMIT:") => {
                report.cutoff = Some(e.message);
                break;
            }
            Err(e) => return Err(e),
        };
        for step in successors {
            let to = if let Some(&i) = intern.get(&step.state) {
                i
            } else {
                if states.len() >= options.max_states {
                    report.cutoff = Some("max-states reached".into());
                    break 'search;
                }
                if depths[cursor] >= options.max_depth {
                    report.cutoff = Some("max-depth reached with unexplored successors".into());
                    break 'search;
                }
                let i = states.len();
                intern.insert(step.state.clone(), i);
                states.push(step.state);
                graph.edges.push(vec![]);
                actions.push(vec![]);
                depths.push(depths[cursor] + 1);
                prev.push(Some((cursor, graph.edges[cursor].len())));
                i
            };
            graph.edges[cursor].push(Edge {
                to,
                action: step.action.id.clone(),
                fair: step.action.fair,
            });
            actions[cursor].push(step.action);
        }
        cursor += 1;
    }
    report.states = states.len();
    report.edges = graph.edges.iter().map(Vec::len).sum();
    report.complete = cursor == states.len() && report.cutoff.is_none() && !failure;
    if failure {
        report.status = Status::Violated;
        return Ok(report);
    }
    if !report.complete {
        return Ok(report);
    }
    // The graph is closed before any liveness pass claims completeness.
    for (i, c) in claims.iter().enumerate() {
        if c.kind == ClaimKind::Invariant {
            report.claims[i].result = "VERIFIED_IN_SCOPE".into();
            continue;
        }
        if c.kind == ClaimKind::Cover {
            if report.claims[i].witness.is_none() {
                report.claims[i].result = "UNREACHABLE".into();
            }
            continue;
        }
        let clauses = temporal::clauses(p, &states[0], &c.body)?;
        if clauses.is_empty() {
            report.claims[i].note = Some("vacuous: empty temporal quantifier domain".into());
        }
        let mut violated = false;
        let mut inconclusive = false;
        for (j, clause) in clauses.iter().enumerate() {
            let checked = temporal::evaluate(p, &states, &graph, clause, &budget);
            let witness = match checked {
                Ok(w) => w,
                Err(e) if e.message.starts_with("LIMIT:") => {
                    report.cutoff = Some(e.message);
                    inconclusive = true;
                    break;
                }
                Err(e) => return Err(e),
            };
            if let Some(walk) = witness {
                let trace =
                    Trace::from_walk(source, p, c, Some(j), &walk, &graph, &states, &actions);
                trace.validate(source, p)?;
                report.claims[i].result = "VIOLATED".into();
                report.claims[i].witness = Some(trace);
                violated = true;
                break;
            }
            if clause.kind == temporal::Kind::Response
                && !states
                    .iter()
                    .map(|s| p.predicate(&clause.p, &clause.env, s))
                    .collect::<Result<Vec<_>>>()?
                    .iter()
                    .any(|x| *x)
            {
                report.claims[i].note =
                    Some("vacuous: a response antecedent is never reached".into());
            }
        }
        if !violated && !inconclusive {
            report.claims[i].result = "VERIFIED_IN_SCOPE".into();
        }
    }
    report.status = if report.claims.iter().any(|c| c.result == "VIOLATED") {
        Status::Violated
    } else if report.claims.iter().any(|c| c.result == "INCONCLUSIVE") {
        Status::Inconclusive
    } else {
        Status::VerifiedInScope
    };
    Ok(report)
}

/// Build a witness for an already-known state using the checker predecessor tree.
pub fn prefix(graph: &Graph, parents: &[Option<(usize, usize)>], target: usize) -> Walk {
    graph::backtrack(graph, parents, target)
}

//! Breadth-first exploration with exact state equality and truthful cutoff reporting.
use crate::{
    graph::{self, Budget, Edge, Graph, Walk},
    model::Program,
    semantics::{Action, Env, State},
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
    pub spawn_bounds: std::collections::BTreeMap<String, usize>,
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
    let claims: Vec<_> = p
        .model
        .claims
        .iter()
        .filter(|c| options.property.as_ref().is_none_or(|n| n == &c.name))
        .collect();
    let mut report = Report {
        status: Status::Inconclusive,
        check: p.check.name.clone(),
        weak_progress: p.check.fair,
        assumptions: vec![
            "finite one-shot external send workload; optional inputs may remain unsubmitted".into(),
            format!(
                "finite FIFO mailboxes (capacity {} per address), atomic state-in/state-out turns; outgoing sends become visible with state commit; retained state is not durable",
                p.check.mailbox_bound.expect("validated mailbox bound")
            ),
            "processing is fault-free; weak progress, when declared, prevents starvation of continuously enabled mailboxes; optional inputs are not forced".into(),
            "the engine does not inject loss, duplication, transport failures, timeouts, restarts, external I/O, or persistence; represent such behavior explicitly in the model".into(),
            if let Some(bound) = p.check.message_bound {
                format!(
                    "message observations: {bound} lifetime slots per actor declaration, including external sends; no slot reuse; exhaustion is inconclusive"
                )
            } else {
                "no lifetime message history selected; external input observations remain finite"
                    .into()
            },
            "finite choose alternatives are exhaustive and unfair; weak mailbox progress does not force any choice outcome".into(),
            "actor definitions create no instances; main constructs one deterministic initial state; main and handler spawns share lifetime bounds, never reuse identities; exhaustion is inconclusive".into(),
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
        input_slots: 0,
        spawn_bounds: p.check.spawn_bounds.clone(),
        not_checked: p
            .model
            .claims
            .iter()
            .filter(|c| options.property.as_ref().is_some_and(|n| n != &c.name))
            .map(|c| c.name.clone())
            .collect(),
    };
    let initial = match p.initial_messages(Some(&budget)) {
        Ok(initial) => initial,
        Err(error) if error.message.starts_with("LIMIT:") => {
            report.cutoff = Some(error.message);
            return Ok(report);
        }
        Err(error) => return Err(error),
    };
    report.input_slots = initial.inputs.len();
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
    let observe = |index: usize,
                   report: &mut Report,
                   states: &[State],
                   graph: &Graph,
                   prev: &graph::Predecessors,
                   actions: &[Vec<Action>]|
     -> Result<bool> {
        for (i, c) in claims.iter().enumerate() {
            if c.kind == ClaimKind::Property {
                continue;
            }
            let value = p.predicate(&c.body, &Env::new(), &states[index])?;
            if c.kind == ClaimKind::Invariant && !value {
                let walk = graph::backtrack(graph, prev, index);
                let trace = Trace::from_walk(source, p, c, None, &walk, graph, states, actions);
                trace.validate(source, p)?;
                report.claims[i].result = "VIOLATED".into();
                report.claims[i].witness = Some(trace);
                return Ok(true);
            }
            if c.kind == ClaimKind::Cover && value && report.claims[i].witness.is_none() {
                let walk = graph::backtrack(graph, prev, index);
                let trace = Trace::from_walk(source, p, c, None, &walk, graph, states, actions);
                trace.validate(source, p)?;
                report.claims[i].result = "REACHED".into();
                report.claims[i].witness = Some(trace);
            }
        }
        Ok(false)
    };
    'search: while cursor < states.len() {
        if let Err(e) = budget.poll() {
            report.cutoff = Some(e.message);
            break;
        }
        if cursor == 0 && observe(0, &mut report, &states, &graph, &prev, &actions)? {
            failure = true;
            break;
        }
        let successors = match p.message_successors(&states[cursor], Some(&budget), None) {
            Ok(s) => s,
            Err(e) if e.message.starts_with("LIMIT:") => {
                report.cutoff = Some(e.message);
                break;
            }
            Err(e) => return Err(e),
        };
        for step in successors {
            let fresh = !intern.contains_key(&step.state);
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
            // Observe discovered states immediately: a later sibling hitting a
            // search budget must not hide an already available finite witness.
            if fresh && observe(to, &mut report, &states, &graph, &prev, &actions)? {
                failure = true;
                break 'search;
            }
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
        let mut unreached_antecedents = 0;
        let mut response_clauses = 0;
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
            if clause.kind == temporal::Kind::Response {
                response_clauses += 1;
                if !states
                    .iter()
                    .map(|s| p.predicate(&clause.p, &clause.env, s))
                    .collect::<Result<Vec<_>>>()?
                    .iter()
                    .any(|x| *x)
                {
                    unreached_antecedents += 1;
                }
            }
        }
        if !violated && !inconclusive {
            report.claims[i].result = "VERIFIED_IN_SCOPE".into();
            if unreached_antecedents > 0 {
                report.claims[i].note = Some(format!(
                    "{unreached_antecedents} of {response_clauses} response clauses have unreached antecedents (including unused observation slots)"
                ));
            }
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

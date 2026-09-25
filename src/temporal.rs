//! A checked temporal fragment, implemented by reachability and fair recurrent SCCs.
use crate::{
    graph::{self, Budget, Graph, Walk},
    model::Program,
    semantics::{Env, State},
    syntax::*,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Always,
    Eventually,
    Response,
    Until,
    Recurrence,
    Stabilization,
    Persistence,
}
#[derive(Clone, Debug)]
pub struct Clause {
    pub expr: Expr,
    pub env: Env,
    pub kind: Kind,
    pub p: Expr,
    pub q: Option<Expr>,
}
pub fn clauses(program: &Program, initial: &State, expr: &Expr) -> Result<Vec<Clause>> {
    fn expand(p: &Program, s: &State, e: &Expr, env: &Env, out: &mut Vec<Clause>) -> Result<()> {
        if out.len() > 4096 {
            return Err(Error::new(
                e.span,
                "temporal expansion exceeds 4096 clauses",
            ));
        }
        match &e.kind {
            ExprKind::Quant {
                all: true,
                var,
                domain,
                body,
            } => {
                for v in p.collection(p.eval(domain, env, s)?, s, domain.span)? {
                    let mut env = env.clone();
                    env.insert(var.clone(), v);
                    expand(p, s, body, &env, out)?;
                }
                return Ok(());
            }
            ExprKind::Binary(op, a, b) if op == "&&" => {
                expand(p, s, a, env, out)?;
                expand(p, s, b, env, out)?;
                return Ok(());
            }
            _ => {}
        }
        let (kind, a, b) = match &e.kind {
            ExprKind::Binary(op, a, b) if op == "leads_to" => {
                (Kind::Response, *a.clone(), Some(*b.clone()))
            }
            ExprKind::Binary(op, a, b) if op == "until" => {
                (Kind::Until, *a.clone(), Some(*b.clone()))
            }
            ExprKind::Unary(op, x) => match &x.kind {
                ExprKind::Unary(inner, pred) if op == "always" && inner == "eventually" => {
                    (Kind::Recurrence, *pred.clone(), None)
                }
                ExprKind::Unary(inner, pred) if op == "eventually" && inner == "always" => {
                    (Kind::Stabilization, *pred.clone(), None)
                }
                ExprKind::Binary(imp, a, b)
                    if op == "always" && imp == "implies" && b.temporal() =>
                {
                    let ExprKind::Unary(_, q) = &b.kind else {
                        return Err(Error::new(e.span, "invalid persistence formula"));
                    };
                    (Kind::Persistence, *a.clone(), Some(*q.clone()))
                }
                _ => (
                    if op == "always" {
                        Kind::Always
                    } else {
                        Kind::Eventually
                    },
                    *x.clone(),
                    None,
                ),
            },
            _ => return Err(Error::new(e.span, "unsupported temporal clause")),
        };
        out.push(Clause {
            expr: e.clone(),
            env: env.clone(),
            kind,
            p: a,
            q: b,
        });
        Ok(())
    }
    let mut out = vec![];
    expand(program, initial, expr, &Env::new(), &mut out)?;
    Ok(out)
}
pub fn evaluate(
    program: &Program,
    states: &[State],
    g: &Graph,
    c: &Clause,
    budget: &Budget,
) -> Result<Option<Walk>> {
    let mut ps = vec![];
    let mut qs = vec![];
    for s in states {
        budget.poll()?;
        ps.push(program.predicate(&c.p, &c.env, s)?);
        qs.push(if let Some(q) = &c.q {
            program.predicate(q, &c.env, s)?
        } else {
            false
        });
    }
    counterexample(g, c.kind, &ps, &qs, budget)
}
/// Graph-only kernel: independent of the language evaluator and Cloudflare semantics.
pub fn counterexample(
    g: &Graph,
    kind: Kind,
    p: &[bool],
    q: &[bool],
    budget: &Budget,
) -> Result<Option<Walk>> {
    let n = g.edges.len();
    let all = vec![true; n];
    let not_p: Vec<_> = p.iter().map(|x| !x).collect();
    let not_q: Vec<_> = q.iter().map(|x| !x).collect();
    let (reachable, prev) = graph::reach(g, &[0], &all, budget)?;
    let prefix = |v| graph::backtrack(g, &prev, v);
    let add_prefix = |w: Walk| {
        let mut pre = prefix(w.start);
        let len = pre.steps.len();
        pre.steps.extend(w.steps);
        pre.loop_start = w.loop_start.map(|i| i + len);
        pre
    };
    match kind {
        Kind::Always => Ok((0..n).find(|&i| reachable[i] && !p[i]).map(prefix)),
        Kind::Eventually => graph::fair_lasso(g, &[0], &not_p, None, budget),
        Kind::Response => {
            let roots: Vec<_> = (0..n).filter(|&i| reachable[i] && p[i] && !q[i]).collect();
            Ok(graph::fair_lasso(g, &roots, &not_q, None, budget)?.map(add_prefix))
        }
        Kind::Until => {
            let (seen, restricted) = graph::reach(g, &[0], &not_q, budget)?;
            if let Some(v) = (0..n).find(|&i| seen[i] && !p[i]) {
                return Ok(Some(graph::backtrack(g, &restricted, v)));
            }
            graph::fair_lasso(g, &[0], &not_q, None, budget)
        }
        Kind::Recurrence => {
            let roots: Vec<_> = (0..n).filter(|&i| reachable[i] && not_p[i]).collect();
            Ok(graph::fair_lasso(g, &roots, &not_p, None, budget)?.map(add_prefix))
        }
        Kind::Stabilization => graph::fair_lasso(g, &[0], &all, Some(&not_p), budget),
        Kind::Persistence => {
            let roots: Vec<_> = (0..n).filter(|&i| reachable[i] && p[i]).collect();
            let (seen, suffix) = graph::reach(g, &roots, &all, budget)?;
            if let Some(v) = (0..n).find(|&i| seen[i] && !q[i]) {
                Ok(Some(add_prefix(graph::backtrack(g, &suffix, v))))
            } else {
                Ok(None)
            }
        }
    }
}

/// Independent fixed-point interpretation over a supplied lasso, used to validate artifacts.
/// Least fixed points for F/U and greatest fixed points for G; no SCC reuse.
pub fn on_trace(
    program: &Program,
    states: &[State],
    loop_start: Option<usize>,
    e: &Expr,
    env: &Env,
) -> Result<bool> {
    let n = states.len();
    if n == 0 {
        return Err(Error::new(e.span, "empty trace"));
    }
    let next = |i: usize| {
        if i + 1 < n {
            i + 1
        } else {
            loop_start.unwrap_or(n - 1)
        }
    };
    fn solve(mut values: Vec<bool>, step: impl Fn(&[bool], usize) -> bool) -> Vec<bool> {
        loop {
            let new: Vec<_> = (0..values.len()).map(|i| step(&values, i)).collect();
            if new == values {
                return values;
            }
            values = new;
        }
    }
    fn values(
        program: &Program,
        states: &[State],
        e: &Expr,
        env: &Env,
        next: &impl Fn(usize) -> usize,
    ) -> Result<Vec<bool>> {
        let n = states.len();
        if !e.temporal() {
            return states
                .iter()
                .map(|s| program.predicate(e, env, s))
                .collect();
        }
        match &e.kind {
            ExprKind::Unary(op, x) => {
                let xs = values(program, states, x, env, next)?;
                match op.as_str() {
                    "always" => Ok(solve(vec![true; n], |v, i| xs[i] && v[next(i)])),
                    "eventually" => Ok(solve(vec![false; n], |v, i| xs[i] || v[next(i)])),
                    _ => Err(Error::new(e.span, "unsupported temporal negation")),
                }
            }
            ExprKind::Binary(op, a, b) => {
                let a = values(program, states, a, env, next)?;
                let b = values(program, states, b, env, next)?;
                match op.as_str() {
                    "leads_to" => {
                        let f = solve(vec![false; n], |v, i| b[i] || v[next(i)]);
                        Ok(solve(vec![true; n], |v, i| (!a[i] || f[i]) && v[next(i)]))
                    }
                    "until" => Ok(solve(vec![false; n], |v, i| b[i] || (a[i] && v[next(i)]))),
                    "implies" => Ok((0..n).map(|i| !a[i] || b[i]).collect()),
                    "&&" => Ok((0..n).map(|i| a[i] && b[i]).collect()),
                    _ => Err(Error::new(e.span, "unsupported temporal connective")),
                }
            }
            _ => Err(Error::new(e.span, "expected expanded temporal clause")),
        }
    }
    Ok(values(program, states, e, env, &next)?[0])
}

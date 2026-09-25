//! Exact native graph algorithms. Fairness uses enabledness in the ORIGINAL graph.
use crate::syntax::{Error, Result, Span};
use std::{
    collections::{BTreeSet, VecDeque},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Edge {
    pub to: usize,
    pub action: String,
    pub fair: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub edges: Vec<Vec<Edge>>,
}
#[derive(Clone, Debug, Default)]
pub struct Walk {
    pub start: usize,
    pub steps: Vec<(usize, usize)>,
    pub loop_start: Option<usize>,
}
impl Walk {
    pub fn end(&self, g: &Graph) -> usize {
        self.steps
            .last()
            .map(|(s, e)| g.edges[*s][*e].to)
            .unwrap_or(self.start)
    }
    pub fn append(&mut self, edges: Vec<(usize, usize)>) {
        self.steps.extend(edges);
    }
}
pub struct Budget {
    pub start: Instant,
    pub timeout: Duration,
}
impl Budget {
    pub fn new(timeout: Duration) -> Self {
        Self {
            start: Instant::now(),
            timeout,
        }
    }
    pub fn poll(&self) -> Result<()> {
        if self.start.elapsed() >= self.timeout {
            Err(Error::new(Span::default(), "LIMIT: timeout"))
        } else {
            Ok(())
        }
    }
}
/// Multi-source BFS, including roots. No frontier state is treated as terminal.
pub type Predecessors = Vec<Option<(usize, usize)>>;

pub fn reach(
    g: &Graph,
    roots: &[usize],
    allowed: &[bool],
    budget: &Budget,
) -> Result<(Vec<bool>, Predecessors)> {
    let mut seen = vec![false; g.edges.len()];
    let mut prev = vec![None; g.edges.len()];
    let mut q = VecDeque::new();
    for &r in roots {
        if allowed[r] && !seen[r] {
            seen[r] = true;
            q.push_back(r);
        }
    }
    while let Some(v) = q.pop_front() {
        budget.poll()?;
        for (i, e) in g.edges[v].iter().enumerate() {
            if allowed[e.to] && !seen[e.to] {
                seen[e.to] = true;
                prev[e.to] = Some((v, i));
                q.push_back(e.to);
            }
        }
    }
    Ok((seen, prev))
}
pub fn backtrack(g: &Graph, prev: &[Option<(usize, usize)>], mut target: usize) -> Walk {
    let mut steps = vec![];
    while let Some((s, e)) = prev[target] {
        steps.push((s, e));
        target = s;
    }
    steps.reverse();
    let walk = Walk {
        start: target,
        steps,
        loop_start: None,
    };
    debug_assert!(walk.steps.iter().all(|(s, e)| *e < g.edges[*s].len()));
    walk
}
pub fn path(
    g: &Graph,
    start: usize,
    end: usize,
    allowed: &[bool],
    budget: &Budget,
) -> Result<Vec<(usize, usize)>> {
    let (seen, prev) = reach(g, &[start], allowed, budget)?;
    if !seen[end] {
        return Err(Error::new(
            Span::default(),
            "internal: missing witness path",
        ));
    }
    Ok(backtrack(g, &prev, end).steps)
}
/// Delegate SCC decomposition to petgraph's non-recursive Kosaraju implementation.
fn sccs(g: &Graph, allowed: &[bool], budget: &Budget) -> Result<Vec<Vec<usize>>> {
    let mut graph = petgraph::graph::DiGraph::<usize, ()>::new();
    let nodes: Vec<_> = (0..g.edges.len()).map(|i| graph.add_node(i)).collect();
    for (v, edges) in g.edges.iter().enumerate() {
        budget.poll()?;
        if allowed[v] {
            for e in edges {
                if allowed[e.to] {
                    graph.add_edge(nodes[v], nodes[e.to], ());
                }
            }
        }
    }
    let mut components: Vec<Vec<usize>> = petgraph::algo::kosaraju_scc(&graph)
        .into_iter()
        .map(|xs| {
            xs.into_iter()
                .map(|n| graph[n])
                .filter(|&v| allowed[v])
                .collect::<Vec<_>>()
        })
        .filter(|xs| !xs.is_empty())
        .collect();
    budget.poll()?;
    for xs in &mut components {
        xs.sort_unstable();
    }
    components.sort_by_key(|c| c[0]);
    Ok(components)
}
/// Find an infinite fair suffix in `allowed`, optionally visiting a marked state infinitely often.
/// The returned prefix begins at a root (the caller supplies any unrestricted prefix).
pub fn fair_lasso(
    g: &Graph,
    roots: &[usize],
    allowed: &[bool],
    visit: Option<&[bool]>,
    budget: &Budget,
) -> Result<Option<Walk>> {
    let (reachable, prev) = reach(g, roots, allowed, budget)?;
    let fair_actions: BTreeSet<_> = g
        .edges
        .iter()
        .flatten()
        .filter(|e| e.fair)
        .map(|e| e.action.clone())
        .collect();
    for component in sccs(g, &reachable, budget)? {
        budget.poll()?;
        if component.len() == 1 && !g.edges[component[0]].iter().any(|e| e.to == component[0]) {
            continue;
        }
        if visit.is_some_and(|v| !component.iter().any(|&n| v[n])) {
            continue;
        }
        let mut inside = vec![false; g.edges.len()];
        for &v in &component {
            inside[v] = true;
        }
        let mut required_edges = vec![];
        let mut fair = true;
        for action in &fair_actions {
            // Visiting every SCC vertex witnesses each recurrently disabled action.
            if component
                .iter()
                .any(|&v| !g.edges[v].iter().any(|e| e.fair && &e.action == action))
            {
                continue;
            }
            let edge = component.iter().find_map(|&v| {
                g.edges[v]
                    .iter()
                    .enumerate()
                    .find(|(_, e)| inside[e.to] && e.fair && &e.action == action)
                    .map(|(i, _)| (v, i))
            });
            if let Some(e) = edge {
                required_edges.push(e);
            } else {
                fair = false;
                break;
            }
        }
        if !fair {
            continue;
        }
        let start = component[0];
        let mut walk = backtrack(g, &prev, start);
        walk.loop_start = Some(walk.steps.len());
        for &v in &component {
            walk.append(path(g, walk.end(g), v, &inside, budget)?);
        }
        for (v, e) in required_edges {
            walk.append(path(g, walk.end(g), v, &inside, budget)?);
            walk.steps.push((v, e));
        }
        walk.append(path(g, walk.end(g), start, &inside, budget)?);
        if walk.steps.len() == walk.loop_start.expect("set above") {
            let (i, e) = g.edges[start]
                .iter()
                .enumerate()
                .find(|(_, e)| inside[e.to])
                .expect("cyclic component");
            walk.steps.push((start, i));
            walk.append(path(g, e.to, start, &inside, budget)?);
        }
        return Ok(Some(walk));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn budget() -> Budget {
        Budget::new(Duration::from_secs(10))
    }
    #[test]
    fn fairness_uses_original_enabledness() {
        let g = Graph {
            edges: vec![
                vec![
                    Edge {
                        to: 0,
                        action: "idle".into(),
                        fair: false,
                    },
                    Edge {
                        to: 1,
                        action: "progress".into(),
                        fair: true,
                    },
                ],
                vec![Edge {
                    to: 1,
                    action: "idle".into(),
                    fair: false,
                }],
            ],
        };
        assert!(
            fair_lasso(&g, &[0], &[true, false], None, &budget())
                .unwrap()
                .is_none()
        );
        assert!(
            fair_lasso(&g, &[0], &[true, true], None, &budget())
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn fair_self_edges_are_not_lost() {
        let g = Graph {
            edges: vec![vec![
                Edge {
                    to: 0,
                    action: "idle".into(),
                    fair: false,
                },
                Edge {
                    to: 0,
                    action: "work".into(),
                    fair: true,
                },
            ]],
        };
        let w = fair_lasso(&g, &[0], &[true], None, &budget())
            .unwrap()
            .unwrap();
        assert!(w.steps.contains(&(0, 1)));
    }
    #[test]
    fn intermittently_enabled_action_need_not_run() {
        let g = Graph {
            edges: vec![
                vec![
                    Edge {
                        to: 1,
                        action: "flip".into(),
                        fair: false,
                    },
                    Edge {
                        to: 2,
                        action: "work".into(),
                        fair: true,
                    },
                ],
                vec![Edge {
                    to: 0,
                    action: "flip".into(),
                    fair: false,
                }],
                vec![],
            ],
        };
        assert!(
            fair_lasso(&g, &[0], &[true, true, false], None, &budget())
                .unwrap()
                .is_some()
        );
    }
}

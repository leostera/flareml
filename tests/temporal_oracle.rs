//! An exponential, independent oracle for tiny graphs. It enumerates recurrent
//! edge subsets and uses matrix closure, NOT SCCs or the production witness code.
use flareml::{
    graph::{Budget, Edge, Graph},
    temporal::{Kind, counterexample},
};
use proptest::prelude::*;
use std::time::Duration;

fn closure(g: &Graph, allowed: &[bool]) -> Vec<Vec<bool>> {
    let n = g.edges.len();
    let mut r = vec![vec![false; n]; n];
    for i in 0..n {
        if allowed[i] {
            r[i][i] = true;
            for e in &g.edges[i] {
                if allowed[e.to] {
                    r[i][e.to] = true;
                }
            }
        }
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                r[i][j] |= r[i][k] && r[k][j];
            }
        }
    }
    r
}
fn oracle(g: &Graph, kind: Kind, p: &[bool], q: &[bool]) -> bool {
    let n = g.edges.len();
    let all = closure(g, &vec![true; n]);
    if kind == Kind::Always {
        return (0..n).any(|i| all[0][i] && !p[i]);
    }
    if kind == Kind::Persistence {
        return (0..n).any(|i| all[0][i] && p[i] && (0..n).any(|j| all[i][j] && !q[j]));
    }
    let not_q: Vec<_> = q.iter().map(|v| !v).collect();
    let before_q = closure(g, &not_q);
    if kind == Kind::Until && (0..n).any(|i| before_q[0][i] && !p[i]) {
        return true;
    }
    let edges: Vec<_> = g
        .edges
        .iter()
        .enumerate()
        .flat_map(|(i, es)| es.iter().map(move |e| (i, e)))
        .collect();
    for bits in 1usize..(1usize << edges.len()) {
        let mut sub = Graph {
            edges: vec![vec![]; n],
        };
        let mut members = vec![false; n];
        for (index, (from, e)) in edges.iter().enumerate() {
            if bits & (1 << index) != 0 {
                sub.edges[*from].push((*e).clone());
                members[*from] = true;
                members[e.to] = true;
            }
        }
        let c = closure(&sub, &members);
        let vs: Vec<_> = (0..n).filter(|&i| members[i]).collect();
        if !vs.iter().all(|&i| vs.iter().all(|&j| c[i][j])) {
            continue;
        }
        let fair = edges.iter().filter(|(_, e)| e.fair).all(|(_, action)| {
            let always_enabled = vs.iter().all(|&i| {
                g.edges[i]
                    .iter()
                    .any(|e| e.fair && e.action == action.action)
            });
            !always_enabled
                || sub
                    .edges
                    .iter()
                    .flatten()
                    .any(|e| e.fair && e.action == action.action)
        });
        if !fair {
            continue;
        }
        let target = vs[0];
        let violation = match kind {
            Kind::Eventually => {
                let not_p: Vec<_> = p.iter().map(|v| !v).collect();
                vs.iter().all(|&i| !p[i]) && closure(g, &not_p)[0][target]
            }
            Kind::Response => {
                vs.iter().all(|&i| !q[i])
                    && (0..n).any(|i| all[0][i] && p[i] && !q[i] && before_q[i][target])
            }
            Kind::Until => vs.iter().all(|&i| !q[i]) && before_q[0][target],
            Kind::Recurrence => all[0][target] && vs.iter().all(|&i| !p[i]),
            Kind::Stabilization => all[0][target] && vs.iter().any(|&i| !p[i]),
            _ => false,
        };
        if violation {
            return true;
        }
    }
    false
}
const KINDS: [Kind; 7] = [
    Kind::Always,
    Kind::Eventually,
    Kind::Response,
    Kind::Until,
    Kind::Recurrence,
    Kind::Stabilization,
    Kind::Persistence,
];

fn graph(adjacency: u8, fair: u8) -> Graph {
    let mut g = Graph {
        edges: vec![vec![], vec![]],
    };
    for i in 0..2 {
        g.edges[i].push(Edge {
            to: i,
            action: "idle".into(),
            fair: false,
        });
        for j in 0..2 {
            if adjacency & (1 << (2 * i + j)) != 0 {
                g.edges[i].push(Edge {
                    to: j,
                    action: format!("work:{i}"),
                    fair: fair & (1 << i) != 0,
                });
            }
        }
    }
    g
}

// Validate produced evidence independently of SCC selection: edge continuity,
// fairness in the original graph, and fixed-point temporal interpretation.
fn validate_walk(g: &Graph, kind: Kind, p: &[bool], q: &[bool], w: &flareml::graph::Walk) {
    use flareml::{
        semantics::{Env, State, Value},
        syntax, temporal,
    };
    use std::collections::{BTreeMap, BTreeSet};
    let mut current = 0;
    assert_eq!(w.start, current);
    let mut visited = vec![current];
    for &(from, edge) in &w.steps {
        assert_eq!(from, current);
        current = g.edges[from][edge].to;
        visited.push(current);
    }
    if let Some(start) = w.loop_start {
        assert!(start < w.steps.len());
        assert_eq!(visited[start], current);
        let actions: BTreeSet<_> = g
            .edges
            .iter()
            .flatten()
            .filter(|e| e.fair)
            .map(|e| &e.action)
            .collect();
        for action in actions {
            if visited[start..w.steps.len()]
                .iter()
                .all(|&v| g.edges[v].iter().any(|e| e.fair && &e.action == action))
            {
                assert!(
                    w.steps[start..]
                        .iter()
                        .any(|&(v, i)| g.edges[v][i].fair && &g.edges[v][i].action == action)
                );
            }
        }
        visited.pop();
    }
    let formula = match kind {
        Kind::Always => "always p_view()",
        Kind::Eventually => "eventually p_view()",
        Kind::Response => "p_view() leads_to q_view()",
        Kind::Until => "p_view() until q_view()",
        Kind::Recurrence => "always eventually p_view()",
        Kind::Stabilization => "eventually always p_view()",
        Kind::Persistence => "always (p_view() implies always q_view())",
    };
    let source = format!(
        "actor P {{ init(): Bool {{ false }} handle_message(s: Bool, m: unit): Bool {{ s }} }} actor Q {{ init(): Bool {{ false }} handle_message(s: Bool, m: unit): Bool {{ s }} }} let p_view = (): Bool {{ forall (x in instances(P)) {{ x.state == Some(true) }} }} let q_view = (): Bool {{ forall (x in instances(Q)) {{ x.state == Some(true) }} }} property \"p\" {{ {formula} }} check C {{ spawn_bound P = 1 spawn_bound Q = 1 mailbox_bound = 1 main {{ spawn(P); spawn(Q) }} }}"
    );
    let expression = syntax::parse(&source).unwrap().claims[0].body.clone();
    let program = flareml::compile(&source, None).unwrap();
    let states: Vec<_> = visited
        .iter()
        .map(|&v| State {
            spawned: BTreeMap::from([
                ("P".into(), vec![Value::Bool(p[v])]),
                ("Q".into(), vec![Value::Bool(q[v])]),
            ]),
            ..State::default()
        })
        .collect();
    assert!(
        !temporal::on_trace(&program, &states, w.loop_start, &expression, &Env::new()).unwrap()
    );
}

#[test]
fn exhaust_all_two_state_graphs_predicates_and_fairness() {
    for adjacency in 0..16 {
        for fair in 0..4 {
            let g = graph(adjacency, fair);
            for pred in 0..4 {
                for consequent in 0..4 {
                    let p: Vec<_> = (0..2).map(|i| pred & (1 << i) != 0).collect();
                    let q: Vec<_> = (0..2).map(|i| consequent & (1 << i) != 0).collect();
                    for kind in KINDS {
                        let result =
                            counterexample(&g, kind, &p, &q, &Budget::new(Duration::from_secs(5)))
                                .unwrap();
                        assert_eq!(
                            result.is_some(),
                            oracle(&g, kind, &p, &q),
                            "{kind:?} {adjacency} {fair} {pred} {consequent}"
                        );
                        if let Some(w) = result {
                            validate_walk(&g, kind, &p, &q, &w);
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn three_state_graphs_with_shared_action_ids(
        edges in prop::collection::vec((0usize..3, 0usize..3, 0u8..3, any::<bool>()), 0..7),
        pred in 0u8..8, consequent in 0u8..8
    ) {
        let mut g = Graph { edges: (0..3).map(|i| vec![Edge { to: i, action: "idle".into(), fair: false }]).collect() };
        for (from, to, action, fair) in edges { g.edges[from].push(Edge { to, action: format!("a:{action}"), fair }); }
        let p: Vec<_> = (0..3).map(|i| pred & (1 << i) != 0).collect();
        let q: Vec<_> = (0..3).map(|i| consequent & (1 << i) != 0).collect();
        for kind in KINDS {
            let result = counterexample(&g, kind, &p, &q, &Budget::new(Duration::from_secs(5))).unwrap();
            prop_assert_eq!(result.is_some(), oracle(&g, kind, &p, &q), "{:?}; {:?}", kind, g.edges);
            if let Some(w) = result { validate_walk(&g, kind, &p, &q, &w); }
        }
    }
    #[test]
    fn native_agrees_with_recurrent_subset_oracle(adjacency in 0u8..16, fair in 0u8..4, pred in 0u8..4, consequent in 0u8..4){
        let mut g=Graph{edges:vec![vec![],vec![]]};
        for i in 0..2{g.edges[i].push(Edge{to:i,action:"idle".into(),fair:false});for j in 0..2{if adjacency&(1<<(2*i+j))!=0{g.edges[i].push(Edge{to:j,action:format!("work:{i}"),fair:fair&(1<<i)!=0});}}}
        let p:Vec<_>=(0..2).map(|i|pred&(1<<i)!=0).collect();let q:Vec<_>=(0..2).map(|i|consequent&(1<<i)!=0).collect();
        for kind in [Kind::Always,Kind::Eventually,Kind::Response,Kind::Until,Kind::Recurrence,Kind::Stabilization,Kind::Persistence]{
            let result=counterexample(&g,kind,&p,&q,&Budget::new(Duration::from_secs(5))).unwrap();
            prop_assert_eq!(result.is_some(),oracle(&g,kind,&p,&q),"{:?}; edges={:?}; p={:?}; q={:?}",kind,g.edges,p,q);
            if let Some(w)=result{prop_assert_eq!(w.start,0);let mut current=0;let mut visited=vec![0];for (from,edge) in w.steps{prop_assert_eq!(from,current);current=g.edges[from][edge].to;visited.push(current);}if let Some(start)=w.loop_start{prop_assert_eq!(visited[start],current);}}
        }
    }
}

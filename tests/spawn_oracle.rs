//! Independent tiny allocator/scheduler. No production evaluation or transition
//! code is used to construct expected states. Compare every edge, not just verdicts.
use flareml::{
    compile,
    semantics::{State, Value},
    syntax::StmtKind,
};
use std::collections::{BTreeSet, HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Reference {
    // 0 absent input, 1 queued input, 2 committed coordinator invocation.
    parents: [u8; 2],
    // One fresh slot per allocation. Some(parent) is pending work/provenance;
    // None is a completed worker. No independent allocation counter can drift.
    workers: Vec<Option<usize>>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Event {
    Idle,
    Submit(usize),
    Launch(usize),
    Work(usize),
}
fn oracle(s: &Reference, bound: usize) -> Option<Vec<(Event, Reference)>> {
    if s.workers.len() == bound && s.parents.contains(&1) {
        return None;
    }
    let mut out = vec![(Event::Idle, s.clone())];
    for i in 0..2 {
        let mut next = s.clone();
        match s.parents[i] {
            0 => {
                next.parents[i] = 1;
                out.push((Event::Submit(i), next));
            }
            1 => {
                next.parents[i] = 2;
                next.workers.push(Some(i));
                out.push((Event::Launch(i), next));
            }
            _ => {}
        }
    }
    for i in 0..s.workers.len() {
        if s.workers[i].is_some() {
            let mut next = s.clone();
            next.workers[i] = None;
            out.push((Event::Work(i), next));
        }
    }
    Some(out)
}
fn addr(name: &str, key: Value) -> Value {
    Value::Address(name.into(), Box::new(key))
}
fn project(s: &State, sends: &[flareml::syntax::Span; 2]) -> Reference {
    assert_eq!(s.spawned["A"], [Value::Unit]);
    assert_eq!(s.spawned["B"], [Value::Unit]);
    assert!(s.messages.is_empty());
    let parents = std::array::from_fn(|i| {
        let phase = if s.input_processed[i] {
            2
        } else if s.input_submitted[i] {
            1
        } else {
            0
        };
        let queue = &s.mailboxes[&addr(if i == 0 { "A" } else { "B" }, Value::Identity(0))];
        assert_eq!(queue.len(), usize::from(phase == 1));
        if let Some(message) = queue.first() {
            assert_eq!(message.input, Some(i));
            assert_eq!(message.payload, Value::Unit);
        }
        phase
    });
    let workers: Vec<_> = s
        .spawned
        .get("Worker")
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, value)| {
            let queue = &s.mailboxes[&addr("Worker", Value::Identity(i))];
            if *value == Value::Bool(true) {
                assert!(queue.is_empty());
                None
            } else {
                assert_eq!(*value, Value::Bool(false));
                assert_eq!(queue.len(), 1);
                assert_eq!(queue[0].payload, Value::Unit);
                assert_eq!(queue[0].input, None);
                assert_eq!(queue[0].observation, None);
                Some(
                    sends
                        .iter()
                        .position(|span| *span == queue[0].source)
                        .unwrap(),
                )
            }
        })
        .collect();
    assert_eq!(s.mailboxes.len(), workers.len() + 2);
    Reference { parents, workers }
}

#[test]
fn all_interleavings_of_two_coordinators_match_reference_allocation_machine() {
    for bound in 0..=3 {
        for fair in [false, true] {
            let source = format!("actor A {{ handle_message(m: unit): unit {{ let w = spawn(Worker); send(w, ()); }} }}
                actor B {{ handle_message(m: unit): unit {{ let w = spawn(Worker); send(w, ()); }} }}
                actor Worker {{ init(): Bool {{ false }} handle_message(s: Bool, m: unit): Bool {{ true }} }}
                property \"safe\" {{ always true }} check C {{ spawn_bound A = 1 spawn_bound B = 1 spawn_bound Worker = {bound} mailbox_bound = 1
                main {{ let a = spawn(A); let b = spawn(B); inputs {{ once send(a, ()) once send(b, ()) }} }} {} }}", if fair { "fairness { weak runtime.progress }" } else { "" });
            let p = compile(&source, None).unwrap();
            let sends = ["A", "B"].map(|actor| {
                let StmtKind::Expr(e) = &p.functions[&p.actors[actor].handler].body[1].kind else {
                    panic!("send statement")
                };
                e.span
            });
            let initial = p.initial().unwrap();
            let mut seen = HashSet::from([initial.clone()]);
            let mut pending = VecDeque::from([initial]);
            while let Some(state) = pending.pop_front() {
                let reference = project(&state, &sends);
                let Some(expected) = oracle(&reference, bound) else {
                    assert!(
                        p.successors(&state)
                            .unwrap_err()
                            .message
                            .contains("spawn capacity")
                    );
                    continue;
                };
                let actual = p.successors(&state).unwrap();
                let mut edges = BTreeSet::new();
                for step in actual {
                    let event = if step.action.id == "stutter" {
                        Event::Idle
                    } else if let Some(i) = step.action.id.strip_prefix("submit:") {
                        Event::Submit(i.parse().unwrap())
                    } else {
                        let address: Value =
                            serde_json::from_str(step.action.id.strip_prefix("process:").unwrap())
                                .unwrap();
                        match address {
                            Value::Address(name, _) if name == "A" => Event::Launch(0),
                            Value::Address(name, _) if name == "B" => Event::Launch(1),
                            Value::Address(name, key) if name == "Worker" => {
                                let Value::Identity(index) = *key else {
                                    panic!("fresh identity")
                                };
                                Event::Work(index)
                            }
                            _ => panic!("unexpected actor"),
                        }
                    };
                    assert_eq!(
                        step.action.fair,
                        fair && matches!(event, Event::Launch(_) | Event::Work(_))
                    );
                    assert!(step.action.choices.is_empty());
                    if matches!(event, Event::Launch(_)) {
                        assert_eq!(step.action.spawns.len(), 1);
                        assert_eq!(
                            step.action.spawns[0].address,
                            addr("Worker", Value::Identity(reference.workers.len()))
                        );
                        assert_eq!(step.action.spawns[0].initial, Value::Bool(false));
                    } else {
                        assert!(step.action.spawns.is_empty());
                    }
                    edges.insert((event, project(&step.state, &sends)));
                    if seen.insert(step.state.clone()) {
                        pending.push_back(step.state);
                    }
                }
                assert_eq!(
                    edges,
                    expected.into_iter().collect(),
                    "bound={bound}, fair={fair}, {reference:?}"
                );
            }
            assert!(seen.len() > 1);
        }
    }
}

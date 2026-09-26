//! Independent small mailbox reference machine: no FML evaluator or lowered
//! instructions in the oracle. Compare every edge (including stutter/fairness),
//! state, and capacity cutoff for two forwarding actors and repeated payloads.
use flareml::{
    compile,
    semantics::{State, Value},
};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Snapshot {
    submitted: Vec<bool>,
    processed: Vec<bool>,
    // false = Hop, true = Stop. Each Hop sends a Stop to the other actor.
    queues: [Vec<(bool, Option<usize>)>; 2],
}

fn reference(
    s: &Snapshot,
    workload: &[(usize, bool)],
    bound: usize,
    fair: bool,
) -> Result<BTreeSet<(String, bool, Snapshot)>, ()> {
    let mut out = BTreeSet::from([("stutter".into(), false, s.clone())]);
    for (i, &(target, payload)) in workload.iter().enumerate() {
        if !s.submitted[i] {
            if s.queues[target].len() == bound {
                return Err(());
            }
            let mut next = s.clone();
            next.submitted[i] = true;
            next.queues[target].push((payload, Some(i)));
            out.insert((format!("submit:{i}"), false, next));
        }
    }
    for target in 0..2 {
        if let Some(&(payload, input)) = s.queues[target].first() {
            let mut next = s.clone();
            next.queues[target].remove(0);
            if let Some(i) = input {
                next.processed[i] = true;
            }
            if !payload {
                if next.queues[1 - target].len() == bound {
                    return Err(());
                }
                next.queues[1 - target].push((true, None));
            }
            out.insert((format!("process:{target}"), fair, next));
        }
    }
    Ok(out)
}

fn snapshot(s: &State) -> Snapshot {
    let mut queues = [vec![], vec![]];
    for (target, name) in ["A", "B"].iter().enumerate() {
        let address = Value::Address((*name).into(), Box::new(Value::Unit));
        queues[target] = s.mailboxes[&address]
            .iter()
            .map(|m| {
                let Value::Variant(name, args) = &m.payload else {
                    panic!("not a message")
                };
                assert!(args.is_empty());
                (
                    match name.as_str() {
                        "Hop" => false,
                        "Stop" => true,
                        _ => panic!("unknown message"),
                    },
                    m.input,
                )
            })
            .collect();
    }
    Snapshot {
        submitted: s.input_submitted.clone(),
        processed: s.input_processed.clone(),
        queues,
    }
}

#[test]
fn native_scheduler_matches_independent_fifo_machine() {
    let mut compared = 0;
    for fair in [false, true] {
        for bound in 1..=3 {
            for workload in [
                vec![],
                vec![(0, false)],
                vec![(0, false), (0, false)],
                vec![(0, false), (1, false)],
                vec![(1, true), (0, false), (1, false)],
            ] {
                let inputs = workload
                    .iter()
                    .map(|&(a, stop)| {
                        format!(
                            "once send({}, {})",
                            ["A", "B"][a],
                            if stop { "Stop" } else { "Hop" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let source = format!(
                    r#"
type Msg = Hop | Stop
actor A {{ handle_message(msg: Msg): unit {{ match msg {{ | Hop -> send(B, Stop) | Stop -> () }} }} }}
actor B {{ handle_message(msg: Msg): unit {{ match msg {{ | Hop -> send(A, Stop) | Stop -> () }} }} }}
property "reference corpus" {{ always true }}
check C {{ mailbox_bound = {bound} inputs {{ {inputs} }} {} }}
"#,
                    if fair {
                        "fairness { weak runtime.progress }"
                    } else {
                        ""
                    }
                );
                let p = compile(&source, None).unwrap();
                let initial = p.initial().unwrap();
                let expected = Snapshot {
                    submitted: vec![false; workload.len()],
                    processed: vec![false; workload.len()],
                    queues: [vec![], vec![]],
                };
                assert_eq!(snapshot(&initial), expected);
                let mut seen = BTreeSet::new();
                let mut pending = VecDeque::from([initial]);
                while let Some(s) = pending.pop_front() {
                    let key = snapshot(&s);
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    compared += 1;
                    let expected = reference(&key, &workload, bound, fair);
                    match (p.successors(&s), expected) {
                        (Err(error), Err(())) => {
                            assert!(error.message.starts_with("LIMIT: mailbox capacity"))
                        }
                        (Ok(actual), Ok(expected)) => {
                            let actual_set: BTreeSet<_> = actual
                                .iter()
                                .map(|step| {
                                    let id = if step.action.id.starts_with("process:") {
                                        let value: Value = serde_json::from_str(
                                            step.action.id.strip_prefix("process:").unwrap(),
                                        )
                                        .unwrap();
                                        match value {
                                            Value::Address(n, _) => {
                                                format!("process:{}", usize::from(n == "B"))
                                            }
                                            _ => panic!("not an address"),
                                        }
                                    } else {
                                        step.action.id.clone()
                                    };
                                    (id, step.action.fair, snapshot(&step.state))
                                })
                                .collect();
                            assert_eq!(actual_set, expected, "{source}\n{key:?}");
                            pending.extend(actual.into_iter().map(|step| step.state));
                        }
                        (actual, expected) => {
                            panic!("cutoff disagreement: {actual:?} / {expected:?}")
                        }
                    }
                }
            }
        }
    }
    assert!(compared > 300, "only compared {compared} states");
}

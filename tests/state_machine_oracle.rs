//! Exhaust all 729 deterministic three-state/two-message transition tables.
//! Compare each reachable edge to a tiny independent machine (no AST/evaluator
//! in the reference), including two optional inputs and their FIFO identities.
use flareml::{
    compile,
    semantics::{State, Value},
};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Snapshot {
    phase: usize,
    submitted: [bool; 2],
    processed: [bool; 2],
    queue: Vec<usize>,
}
fn snapshot(s: &State) -> Snapshot {
    let Value::Variant(phase, _) = &s.spawned["Machine"][0] else {
        panic!("not a phase")
    };
    Snapshot {
        phase: phase.strip_prefix('S').unwrap().parse().unwrap(),
        submitted: s.input_submitted.clone().try_into().unwrap(),
        processed: s.input_processed.clone().try_into().unwrap(),
        queue: s
            .mailboxes
            .values()
            .next()
            .unwrap()
            .iter()
            .map(|e| e.input.unwrap())
            .collect(),
    }
}
fn reference(
    s: &Snapshot,
    table: &[[usize; 2]; 3],
    payloads: [usize; 2],
    fair: bool,
) -> BTreeSet<(String, bool, Snapshot)> {
    let mut out = BTreeSet::from([("stutter".into(), false, s.clone())]);
    for i in 0..2 {
        if !s.submitted[i] {
            let mut next = s.clone();
            next.submitted[i] = true;
            next.queue.push(i);
            out.insert((format!("submit:{i}"), false, next));
        }
    }
    if let Some(&i) = s.queue.first() {
        let mut next = s.clone();
        next.queue.remove(0);
        next.phase = table[s.phase][payloads[i]];
        next.processed[i] = true;
        out.insert(("process".into(), fair, next));
    }
    out
}
#[test]
fn every_three_state_transition_table_matches_reference() {
    let mut compared = 0;
    for mut encoding in 0..729 {
        let mut table = [[0; 2]; 3];
        for row in &mut table {
            for target in row {
                *target = encoding % 3;
                encoding /= 3;
            }
        }
        for (payloads, fair) in [([0, 1], true), ([0, 0], false)] {
            let arms = table
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    format!(
                        "| S{i} -> match message {{ | M0 -> S{} | M1 -> S{} }}",
                        row[0], row[1]
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let source = format!(
                r#"
type Phase = S0 | S1 | S2
type Message = M0 | M1
actor Machine {{
  init(): Phase {{ S0 }}
  handle_message(state: Phase, message: Message): Phase {{ match state {{ {arms} }} }}
}}
property "well formed" {{ always true }}
check C {{ spawn_bound Machine = 1 mailbox_bound = 2 main {{ let machine = spawn(Machine); inputs {{ once send(machine, M{}) once send(machine, M{}) }} }} {} }}
"#,
                payloads[0],
                payloads[1],
                if fair {
                    "fairness { weak runtime.progress }"
                } else {
                    ""
                }
            );
            let p = compile(&source, None).unwrap();
            let initial = p.initial().unwrap();
            assert_eq!(
                snapshot(&initial),
                Snapshot {
                    phase: 0,
                    submitted: [false; 2],
                    processed: [false; 2],
                    queue: vec![]
                }
            );
            let mut seen = BTreeSet::new();
            let mut pending = VecDeque::from([initial]);
            while let Some(s) = pending.pop_front() {
                let key = snapshot(&s);
                if !seen.insert(key.clone()) {
                    continue;
                }
                compared += 1;
                let steps = p.successors(&s).unwrap();
                let actual = steps
                    .iter()
                    .map(|step| {
                        (
                            if step.action.id.starts_with("process:") {
                                "process".into()
                            } else {
                                step.action.id.clone()
                            },
                            step.action.fair,
                            snapshot(&step.state),
                        )
                    })
                    .collect();
                assert_eq!(
                    reference(&key, &table, payloads, fair),
                    actual,
                    "{table:?} {key:?}"
                );
                pending.extend(steps.into_iter().map(|s| s.state));
            }
        }
    }
    assert!(compared > 10_000, "only compared {compared} states");
}

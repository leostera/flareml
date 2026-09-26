//! Independent setup expectations: direct positional vectors and FIFO sequences,
//! not the production evaluator, allocator, or successor generator.
use flareml::{compile, semantics::Value};

#[test]
fn finite_setups_match_explicit_population_and_workload_vectors() {
    for count in 0..=4 {
        for mask in 0..(1usize << count) {
            for bound in 0..=4 {
                let mut setup = String::new();
                let mut inputs = String::new();
                let expected: Vec<_> = (0..count).map(|i| mask & (1 << i) != 0).collect();
                for (i, bit) in expected.iter().enumerate() {
                    setup.push_str(&format!(
                        "let w{i} = allocate({bit}); send(w{i}, true); send(w{i}, false); "
                    ));
                    inputs.push_str(&format!("once send(w{i}, {bit}) "));
                }
                let source = format!(
                    r#"
actor Worker {{ init(seed: Bool): Bool {{ !seed }} handle_message(state: Bool, m: Bool): Bool {{ m }} }}
let allocate = (seed: Bool): Actor<Worker> {{ spawn(Worker, !seed) }}
property "ok" {{ always true }}
check C {{ spawn_bound Worker = {bound} mailbox_bound = 2 message_bound = {}
  main {{ {setup} inputs {{ {inputs} }} }}
}}
"#,
                    (2 * count).max(1)
                );
                let program = compile(&source, None).unwrap();
                if count > bound {
                    assert!(
                        program
                            .initial()
                            .unwrap_err()
                            .message
                            .contains("spawn capacity")
                    );
                    continue;
                }
                let initial = program.initial().unwrap();
                assert_eq!(initial.mailboxes.len(), count);
                assert_eq!(initial.inputs.len(), count);
                assert_eq!(initial.input_submitted, vec![false; count]);
                assert_eq!(initial.input_processed, vec![false; count]);
                assert_eq!(initial.spawned.get("Worker").map_or(0, Vec::len), count);
                assert_eq!(
                    initial.messages.get("Worker").map_or(0, Vec::len),
                    2 * count
                );
                for (i, bit) in expected.iter().enumerate() {
                    let address = Value::Address("Worker".into(), Box::new(Value::Identity(i)));
                    assert_eq!(initial.spawned["Worker"][i], Value::Bool(*bit));
                    assert_eq!(initial.inputs[i].target, address);
                    assert_eq!(initial.inputs[i].payload, Value::Bool(*bit));
                    let queue = &initial.mailboxes[&address];
                    assert_eq!(queue.len(), 2);
                    for (j, payload) in [true, false].into_iter().enumerate() {
                        assert_eq!(queue[j].payload, Value::Bool(payload));
                        assert_eq!(queue[j].input, None);
                        assert_eq!(queue[j].observation, Some(2 * i + j));
                        let observed = &initial.messages["Worker"][2 * i + j];
                        assert_eq!(observed.target, address);
                        assert_eq!(observed.payload, Value::Bool(payload));
                        assert!(!observed.external && !observed.processed);
                    }
                }
            }
        }
    }
}

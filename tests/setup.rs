use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Value,
};

const MODEL: &str = r#"
actor Worker {
  init(value: Bool): Bool { value }
  handle_message(state: Bool, message: Bool): Bool { message }
}
check Test {
  spawn_bound Worker = 2
  mailbox_bound = 2
  message_bound = 2
  main {
    let first = spawn(Worker, false);
    let second = spawn(Worker, true);
    send(first, true);
    inputs { once send(second, false) }
  }
  fairness { weak runtime.progress }
}
property "initial work finishes" {
  forall (m in messages(Worker)) { m.sent leads_to m.processed }
}
property "can update both" { reachable(forall (w in instances(Worker)) { w.state == Some(true) }) }
"#;

#[test]
fn deterministic_setup_installs_instances_outbox_and_captured_inputs() {
    let p = compile(MODEL, None).unwrap();
    let initial = p.initial().unwrap();
    assert_eq!(initial, p.initial().unwrap());
    assert_eq!(
        initial.spawned["Worker"],
        [Value::Bool(false), Value::Bool(true)]
    );
    assert_eq!(initial.mailboxes.len(), 2);
    assert_eq!(initial.mailboxes.values().map(Vec::len).sum::<usize>(), 1);
    assert_eq!(initial.inputs.len(), 1);
    assert_eq!(initial.input_submitted, [false]);
    assert_eq!(initial.input_processed, [false]);
    assert_eq!(
        initial.inputs[0].target,
        Value::Address("Worker".into(), Box::new(Value::Identity(1)))
    );
    let report = checker::check(MODEL, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::VerifiedInScope, "{report:?}");
    let witness = report.witness().unwrap();
    witness.validate(MODEL, &p).unwrap();
    let mut corrupted = witness.clone();
    corrupted.states[0].inputs[0].payload = Value::Bool(true);
    assert!(corrupted.validate(MODEL, &p).is_err());
}

#[test]
fn declarations_create_nothing_and_discarded_spawn_works() {
    let model = "actor Idle { handle_message(m: unit): unit {} } check C { spawn_bound Idle = 1 mailbox_bound = 1 main {} } property \"ok\" { always(true) }";
    let p = compile(model, None).unwrap();
    assert!(p.initial().unwrap().mailboxes.is_empty());
    let p = compile(&model.replace("main {}", "main { spawn(Idle) }"), None).unwrap();
    let s = p.initial().unwrap();
    assert_eq!(s.mailboxes.len(), 1);
    assert!(s.mailboxes.values().all(Vec::is_empty));
    assert_eq!(p.successors(&s).unwrap().len(), 1);
}

#[test]
fn setup_capacity_is_a_cutoff_not_a_partial_initial_state() {
    for source in [
        MODEL.replace("Worker = 2", "Worker = 1"),
        MODEL.replace(
            "send(first, true);",
            "send(first, true); send(first, true); send(first, true);",
        ),
    ] {
        let p = compile(&source, None).unwrap();
        assert!(p.initial().unwrap_err().message.starts_with("LIMIT:"));
        let report = checker::check(&source, &p, &Options::default()).unwrap();
        assert_eq!(report.status, Status::Inconclusive);
        assert_eq!(report.states, 0);
        assert!(!report.complete);
        assert!(report.witness().is_none());
        assert!(report.claims.iter().all(|c| c.result == "INCONCLUSIVE"));
    }
}

#[test]
fn main_is_deterministic_even_through_helpers_and_bindings_are_local() {
    let base = "actor A { handle_message(m: unit): unit {} } check C { spawn_bound A = 1 mailbox_bound = 1 main { BODY } } property \"ok\" { always(true) }";
    for body in [
        "let x = choose([true, false]);",
        "send(A, ());",
        "let a = spawn(A); let x = instances(A);",
        "let a = spawn(A, true);",
    ] {
        assert!(
            compile(&base.replace("BODY", body), None).is_err(),
            "{body}"
        );
    }
    let source = format!(
        "let coin = (): Bool {{ let c = choose([true, false]); c }} {}",
        base.replace("BODY", "let x = coin();")
    );
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("deterministic")
    );
    let source = format!(
        "{} property \"scope\" {{ always(a == a) }}",
        base.replace("BODY", "let a = spawn(A);")
    );
    assert!(compile(&source, None).is_err());
    for source in [
        MODEL.replace("spawn(Worker, false)", "spawn(Worker)"),
        MODEL.replace("spawn(Worker, false)", "spawn(Worker, ())"),
        MODEL.replace("spawn(Worker, false)", "spawn(Worker, first)"),
        MODEL.replace("main {", "main { Ok(true);"),
    ] {
        assert!(compile(&source, None).is_err(), "{source}");
    }
    let other = format!(
        "{} check Invalid {{ spawn_bound A = 1 mailbox_bound = 1 main {{ let x = choose([true, false]); }} }}",
        base.replace("BODY", "")
    );
    assert!(
        compile(&other, Some("C"))
            .unwrap_err()
            .message
            .contains("deterministic")
    );
}

#[test]
fn old_population_syntax_is_rejected() {
    for source in [
        "spawnable actor A { handle_message(m: unit): unit {} }",
        "actor A(id: Bool) { handle_message(m: unit): unit {} }",
        "check C { mailbox_bound = 1 }",
        "check C { mailbox_bound = 1 main {} inputs {} }",
    ] {
        assert!(compile(source, None).is_err(), "{source}");
    }
}

#[test]
fn selected_check_runs_only_its_own_main() {
    let source = "actor A { handle_message(m: unit): unit {} } check Empty { spawn_bound A = 1 mailbox_bound = 1 main {} } check One { spawn_bound A = 1 mailbox_bound = 1 main { spawn(A) } } property \"ok\" { always(true) }";
    assert!(
        compile(source, Some("Empty"))
            .unwrap()
            .initial()
            .unwrap()
            .mailboxes
            .is_empty()
    );
    assert_eq!(
        compile(source, Some("One"))
            .unwrap()
            .initial()
            .unwrap()
            .mailboxes
            .len(),
        1
    );
}

#[test]
fn setup_and_handlers_share_lifetime_capacity() {
    let source = "actor Worker { handle_message(m: unit): unit { spawn(Worker); } } property \"ok\" { always true } check C { spawn_bound Worker = 2 mailbox_bound = 1 main { let first = spawn(Worker); send(first, ()); } }";
    let p = compile(source, None).unwrap();
    let initial = p.initial().unwrap();
    let step = p
        .successors(&initial)
        .unwrap()
        .into_iter()
        .find(|s| !s.action.spawns.is_empty())
        .unwrap();
    assert_eq!(
        step.action.spawns[0].address,
        Value::Address("Worker".into(), Box::new(Value::Identity(1)))
    );
    assert_eq!(step.state.spawned["Worker"].len(), 2);
    let limited = source.replace("Worker = 2", "Worker = 1");
    let p = compile(&limited, None).unwrap();
    assert_eq!(p.initial().unwrap().spawned["Worker"].len(), 1);
    let report = checker::check(&limited, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert!(report.cutoff.unwrap().contains("spawn capacity"));
}

#[test]
fn deterministic_branches_capture_only_executed_input_registrations() {
    let source = "actor A { handle_message(m: Bool): unit {} } property \"ok\" { always true } check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); match Some(a) { | Some(target) -> { inputs { once send(target, true) }; () } | None -> { inputs { once send(a, true) } } }; inputs { once send(a, false) } } }";
    let state = compile(source, None).unwrap().initial().unwrap();
    assert_eq!(
        state
            .inputs
            .iter()
            .map(|i| i.payload.clone())
            .collect::<Vec<_>>(),
        [Value::Bool(true), Value::Bool(false)]
    );
    assert_eq!(state.inputs[0].target, state.inputs[1].target);
    assert!(state.mailboxes.values().all(Vec::is_empty));
    let bad = source.replace(
        "inputs { once send(a, false) }",
        "inputs { once send(target, false) }",
    );
    assert!(compile(&bad, None).is_err());
}

#[test]
fn setup_deadline_has_no_partial_state_or_fabricated_witness() {
    let p = compile(MODEL, None).unwrap();
    let report = checker::check(
        MODEL,
        &p,
        &Options {
            timeout: std::time::Duration::ZERO,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert_eq!(report.cutoff.as_deref(), Some("LIMIT: timeout"));
    assert_eq!(report.states, 0);
    assert!(report.witness().is_none());
    assert_eq!(
        checker::check(MODEL, &p, &Options::default())
            .unwrap()
            .status,
        Status::VerifiedInScope
    );
}

#[test]
fn initial_snapshot_population_queues_and_inputs_are_reconstructed() {
    let p = compile(MODEL, None).unwrap();
    let report = checker::check(MODEL, &p, &Options::default()).unwrap();
    let trace = report.witness().unwrap();
    for mutation in 0..7 {
        let mut changed = trace.clone();
        let state = &mut changed.states[0];
        match mutation {
            0 => state.spawned.get_mut("Worker").unwrap().swap(0, 1),
            1 => {
                state.inputs.clear();
            }
            2 => {
                state.inputs[0].target =
                    Value::Address("Worker".into(), Box::new(Value::Identity(0)))
            }
            3 => state.inputs[0].source.start += 1,
            4 => state.input_submitted[0] = true,
            5 => state.messages.get_mut("Worker").unwrap()[0].external = true,
            _ => {
                state
                    .mailboxes
                    .values_mut()
                    .find(|q| !q.is_empty())
                    .unwrap()
                    .clear();
            }
        }
        assert!(changed.validate(MODEL, &p).is_err(), "mutation {mutation}");
    }
}

#[test]
fn setup_cutoff_persists_an_inconclusive_report_without_witnesses() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("setup.fml");
    std::fs::write(&source, MODEL.replace("Worker = 2", "Worker = 1")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_fml"))
        .arg("check")
        .arg(&source)
        .args(["--format", "json", "--artifacts-dir"])
        .arg(dir.path().join("runs"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "INCONCLUSIVE");
    assert_eq!(report["states"], 0);
    assert_eq!(report["complete"], false);
    let path = std::path::Path::new(report["artifacts_dir"].as_str().unwrap());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path.join("report.json")).unwrap()).unwrap();
    assert!(saved["witness_files"].as_array().unwrap().is_empty());
    assert_eq!(
        std::fs::read(path.join("model.fml")).unwrap(),
        std::fs::read(source).unwrap()
    );
}

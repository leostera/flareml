use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::{State, Value},
};

fn model(body: &str) -> String {
    format!("actor A {{ init(): Int {{ 0 }} handle_message(s: Int, m: unit): Int {{ {body} }} }}
        actor Sink {{ handle_message(m: Int): unit {{ () }} }}
        property \"safe\" {{ always (A.state <= 6) }}
        property \"changed\" {{ reachable (A.state == 3) }}
        check C {{ domain Int = 0..6 mailbox_bound = 4 inputs {{ once send(A, ()) }} fairness {{ weak runtime.progress }} }}")
}
fn queued(p: &flareml::model::Program) -> State {
    p.successors(&p.initial().unwrap())
        .unwrap()
        .into_iter()
        .find(|s| s.action.id == "submit:0")
        .unwrap()
        .state
}
fn run(source: &str) -> checker::Report {
    checker::check(source, &compile(source, None).unwrap(), &Options::default()).unwrap()
}

#[test]
fn exhaustive_small_candidate_lists_match_independent_cartesian_oracle() {
    // No production evaluator/oracle used to compute expected outcomes.
    for first in [vec![0], vec![0, 1], vec![1, 1], vec![0, 1, 2]] {
        for second in [vec![0], vec![0, 1], vec![1, 1], vec![0, 1, 2]] {
            let source = model(&format!(
                "send(Sink, 6); let a = choose({first:?}); send(Sink, a); let b = choose({second:?}); send(Sink, b); a + b"
            )).replace("mailbox_bound = 4", "mailbox_bound = 4 message_bound = 4");
            let p = compile(&source, None).unwrap();
            let before = queued(&p);
            let steps: Vec<_> = p
                .successors(&before)
                .unwrap()
                .into_iter()
                .filter(|s| s.action.id.starts_with("process:"))
                .collect();
            assert_eq!(steps.len(), first.len() * second.len());
            let sink = Value::Address("Sink".into(), Box::new(Value::Unit));
            for (step, (ai, bi)) in steps
                .iter()
                .zip((0..first.len()).flat_map(|a| (0..second.len()).map(move |b| (a, b))))
            {
                assert_eq!(step.state.actors["A"], Value::Int(first[ai] + second[bi]));
                assert_eq!(
                    step.state.mailboxes[&sink]
                        .iter()
                        .map(|e| e.payload.clone())
                        .collect::<Vec<_>>(),
                    vec![Value::Int(6), Value::Int(first[ai]), Value::Int(second[bi])]
                );
                assert_eq!(
                    step.action
                        .choices
                        .iter()
                        .map(|c| c.candidate)
                        .collect::<Vec<_>>(),
                    vec![ai, bi]
                );
                assert!(step.action.fair);
                assert_eq!(step.action.id, steps[0].action.id);
                assert!(step.state.input_processed[0]);
                assert!(step.state.messages["A"][0].processed);
                assert_eq!(step.state.messages["Sink"].len(), 3);
                assert!(step.state.messages["Sink"].iter().all(|m| !m.processed));
                assert_eq!(
                    step.state.messages["Sink"]
                        .iter()
                        .map(|m| m.payload.clone())
                        .collect::<Vec<_>>(),
                    vec![Value::Int(6), Value::Int(first[ai]), Value::Int(second[bi])]
                );
            }
            assert_eq!(before, queued(&p), "exploration mutated its input");
        }
    }
}

#[test]
fn dependent_choices_and_repeated_helpers_replay_with_encounter_context() {
    let source = format!(
        "let pick = (x: Int): Int {{ send(Sink, x); let selected = choose([x, x + 1]); send(Sink, selected); selected }} {}",
        model("let a = pick(0); let b = pick(a); a + b")
    );
    let p = compile(&source, None).unwrap();
    let turns: Vec<_> = p
        .successors(&queued(&p))
        .unwrap()
        .into_iter()
        .filter(|s| s.action.id.starts_with("process:"))
        .collect();
    let sink = Value::Address("Sink".into(), Box::new(Value::Unit));
    for (step, (a, b)) in turns
        .iter()
        .zip((0..2).flat_map(|a| (a..=a + 1).map(move |b| (a, b))))
    {
        assert_eq!(
            step.state.mailboxes[&sink]
                .iter()
                .map(|m| m.payload.clone())
                .collect::<Vec<_>>(),
            [0, a, a, b].map(Value::Int)
        );
    }
    assert_eq!(turns.len(), 4);
    let report = run(&source);
    assert_eq!(report.status, Status::VerifiedInScope);
    let trace = report.witness().unwrap();
    trace.validate(&source, &p).unwrap();
    let choices = &trace.actions.last().unwrap().choices;
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].span, choices[1].span);
    assert_ne!(choices[0].calls, choices[1].calls);
    assert_eq!((choices[0].encounter, choices[1].encounter), (0, 1));
    for kind in 0..8 {
        let mut bad = trace.clone();
        let choices = &mut bad.actions.last_mut().unwrap().choices;
        match kind {
            0 => {
                choices.pop();
            }
            1 => choices.push(choices[0].clone()),
            2 => choices.swap(0, 1),
            3 => choices[0].candidate = 100,
            4 => choices[0].value = Value::Int(6),
            5 => choices[0].span.start += 1,
            6 => choices[0].calls.clear(),
            _ => choices[1].encounter = 0,
        }
        assert!(bad.validate(&source, &p).is_err(), "mutation {kind}");
    }
    // A different valid alternative must also reproduce the recorded next state.
    let mut bad = trace.clone();
    let c = &mut bad.actions.last_mut().unwrap().choices[1];
    c.candidate = 0;
    c.value = Value::Int(1);
    assert!(bad.validate(&source, &p).is_err());
}

#[test]
fn branch_specific_encounters_and_outboxes_are_isolated() {
    let source = format!(
        "type Decision = Left | Right {}",
        model(
            "let branch = choose([Left, Right]); match branch { | Left -> { let n = choose([0, 1]); send(Sink, n); n } | Right -> { send(Sink, 3); 3 } }"
        )
    );
    let p = compile(&source, None).unwrap();
    let steps: Vec<_> = p
        .successors(&queued(&p))
        .unwrap()
        .into_iter()
        .filter(|s| s.action.id.starts_with("process:"))
        .collect();
    assert_eq!(
        steps
            .iter()
            .map(|s| s.action.choices.len())
            .collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
    let sink = Value::Address("Sink".into(), Box::new(Value::Unit));
    for (step, expected) in steps.iter().zip([0, 1, 3]) {
        assert_eq!(step.state.actors["A"], Value::Int(expected));
        assert_eq!(step.state.mailboxes[&sink].len(), 1);
        assert_eq!(step.state.mailboxes[&sink][0].payload, Value::Int(expected));
    }
}

#[test]
fn malformed_choices_and_effect_escapes_are_rejected() {
    for body in [
        "let x = choose([]); 0",
        "let x = choose([0, true]); 0",
        "let x = choose(0); 0",
        "let xs = [0, 1]; let x = choose(xs); 0",
        "choose([0, 1])",
        "choose([0, 1]); 0",
        "let x = Some(choose([0, 1])); 0",
        "let x = choose([choose([0, 1])]); 0",
        "let x = choose([send(Sink, 0)]); 0",
        "let x = choose([A.state]); 0",
        "let x = choose([inputs(A)]); 0",
        "let x = choose([Ok(0), Err(1)]); 0",
        "let choose = 0; 0",
    ] {
        assert!(compile(&model(body), None).is_err(), "accepted {body}");
    }
    let helper = "let pick = (): Int { let x = choose([0, 1]); x } let indirect = (): Int { let x = pick(); x } ";
    for source in [
        model("0").replace("init(): Int { 0 }", "init(): Int { indirect() }"),
        model("0").replace("A.state <= 6", "indirect() == 0"),
        model("let x = choose([indirect()]); x"),
        model("send(Sink, indirect()); 0"),
        model("let x = [indirect()]; 0"),
        model("0").replace("once send(A, ())", "once send(Sink, indirect())"),
        model("0")
            .replace("actor Sink {", "actor Sink(id: Int) {")
            .replace("once send(A, ())", "once send(Sink.at(indirect()), 0)"),
        model("0").replace("domain Int = 0..6", "domain Int = [indirect()]"),
    ] {
        assert!(
            compile(&format!("{helper}{source}"), None).is_err(),
            "accepted {source}"
        );
    }
    for declaration in [
        "let choose = (): Int { 0 }",
        "type choose = X",
        "type X = choose",
        "actor choose { handle_message(m: unit): unit { () } }",
    ] {
        assert!(compile(&format!("{declaration} {}", model("0")), None).is_err());
    }
}

#[test]
fn choice_result_bindings_still_require_explicit_handling() {
    let source =
        model("let result = choose([Ok(3), Err(0)]); match result { | Ok(x) -> x | Err(x) -> x }");
    let report = run(&source);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(report.claims[1].result, "REACHED");
}

#[test]
fn fairness_does_not_force_a_favorable_choice() {
    let source = "actor A { init(): Bool { false } handle_message(s: Bool, m: unit): Bool { let next = choose([false, true]); send(A, ()); next } } property \"progress\" { (exists (i in inputs(A)) { i.submitted }) leads_to A.state } check C { mailbox_bound = 1 inputs { once send(A, ()) } fairness { weak runtime.progress } }";
    let p = compile(source, None).unwrap();
    let report = run(source);
    assert_eq!(report.status, Status::Violated);
    let trace = report.witness().unwrap();
    let start = trace.loop_start.unwrap();
    assert!(
        trace.actions[start..]
            .iter()
            .any(|a| a.fair && !a.choices.is_empty())
    );
    assert!(
        trace.actions[start..]
            .iter()
            .flat_map(|a| &a.choices)
            .all(|c| c.value == Value::Bool(false))
    );
    trace.validate(source, &p).unwrap();
}

#[test]
fn every_choice_is_checked_for_safety_and_order_does_not_change_verdict() {
    for candidates in ["[0, 3]", "[3, 0]"] {
        let source = model(&format!("let n = choose({candidates}); n"))
            .replace("A.state <= 6", "A.state <= 2");
        let report = run(&source);
        assert_eq!(report.status, Status::Violated);
        let trace = report.witness().unwrap();
        assert_eq!(
            trace.actions.last().unwrap().choices[0].value,
            Value::Int(3)
        );
        trace
            .validate(&source, &compile(&source, None).unwrap())
            .unwrap();
    }
}

#[test]
fn choice_encounter_and_prefix_guards_are_explicit() {
    let body = (0..129)
        .map(|i| format!("let x{i} = choose([false]); "))
        .collect::<String>()
        + "0";
    let r = run(&model(&body));
    assert_eq!(r.status, Status::Inconclusive);
    assert!(r.cutoff.unwrap().contains("128 choice encounters"));
    // One choice with 4096 candidates needs 4097 prefix executions (root + leaves).
    let body = format!("let x = choose([{}]); 0", vec!["0"; 4096].join(","));
    let r = run(&model(&body));
    assert_eq!(r.status, Status::Inconclusive);
    assert!(r.cutoff.unwrap().contains("4096 prefix executions"));
}

#[test]
fn integer_overflow_in_an_alternative_retains_existing_limit_classification() {
    let source = model("let n = choose([0, 9223372036854775807 + 1]); n");
    let p = compile(&source, None).unwrap();
    let report = checker::check(&source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert_eq!(report.cutoff.as_deref(), Some("LIMIT: integer overflow"));
}

#[test]
fn invalid_alternative_and_branch_explosion_never_prove_verification() {
    for body in [
        "let n = choose([0, 7]); n",
        "let decision = choose([Some(0), None]); match decision { | Some(n) -> n | None -> { send(Sink, 0); send(Sink, 0); send(Sink, 0); send(Sink, 0); send(Sink, 0); 0 } }",
    ] {
        let source = model(body);
        let p = compile(&source, None).unwrap();
        let state = queued(&p);
        let before = state.clone();
        assert!(
            p.successors(&state)
                .unwrap_err()
                .message
                .starts_with("LIMIT:")
        );
        assert_eq!(state, before);
        let report = run(&source);
        assert_eq!(report.status, Status::Inconclusive);
        assert!(!report.complete);
    }
    let body = (0..14)
        .map(|i| format!("let x{i} = choose([0, 1]); "))
        .collect::<String>()
        + "0";
    let report = run(&model(&body));
    assert_eq!(report.status, Status::Inconclusive);
    assert!(report.cutoff.unwrap().contains("LIMIT:"));
    // A completed reachability witness from initialization survives a later cutoff.
    let source = model(&body).replace("A.state == 3", "A.state == 0");
    let report = run(&source);
    assert_eq!(report.claims[1].result, "REACHED");
    report
        .witness()
        .unwrap()
        .validate(&source, &compile(&source, None).unwrap())
        .unwrap();
    assert_eq!(
        run(&model("0")).status,
        Status::VerifiedInScope,
        "budget must reset after failure"
    );
}

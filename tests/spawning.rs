use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::{State, Value},
};

fn source(body: &str, bound: usize) -> String {
    format!("actor Parent {{ handle_message(m: unit): unit {{ {body} }} }}
        actor Worker {{ init(): Bool {{ false }} handle_message(s: Bool, m: unit): Bool {{ true }} }}
        property \"consistent\" {{ always (forall (w in instances(Worker)) {{ w.created == (w.reference != None) && w.created == (w.state != None) }}) }}
        property \"done\" {{ reachable (exists (w in instances(Worker)) {{ w.state == Some(true) }}) }}
        property \"future progress\" {{ forall (w in instances(Worker)) {{ w.created leads_to w.state == Some(true) }} }}
        check C {{ spawn_bound Parent = 1 spawn_bound Worker = {bound} mailbox_bound = 2 main {{ let parent = spawn(Parent); inputs {{ once send(parent, ()) }} }} fairness {{ weak runtime.progress }} }}")
}
fn run(s: &str) -> checker::Report {
    checker::check(s, &compile(s, None).unwrap(), &Options::default()).unwrap()
}
fn submitted(p: &flareml::model::Program) -> State {
    p.successors(&p.initial().unwrap())
        .unwrap()
        .into_iter()
        .find(|s| s.action.id == "submit:0")
        .unwrap()
        .state
}
fn address(i: usize) -> Value {
    Value::Address("Worker".into(), Box::new(Value::Identity(i)))
}

#[test]
fn two_spawns_publish_fresh_state_and_mailboxes_atomically() {
    let s = source(
        "let a = spawn(Worker); let b = spawn(Worker); send(a, ()); send(b, ());",
        2,
    )
    .replace("mailbox_bound = 2", "mailbox_bound = 1 message_bound = 2");
    let p = compile(&s, None).unwrap();
    let initial = p.initial().unwrap();
    assert_eq!(initial.mailboxes.len(), 1);
    assert_eq!(initial.spawned["Parent"], [Value::Unit]);
    assert!(!initial.spawned.contains_key("Worker"));
    let before = submitted(&p);
    let steps: Vec<_> = p
        .successors(&before)
        .unwrap()
        .into_iter()
        .filter(|s| s.action.id.starts_with("process:"))
        .collect();
    assert_eq!(steps.len(), 1);
    let turn = &steps[0];
    assert_eq!(
        turn.action
            .spawns
            .iter()
            .map(|s| s.address.clone())
            .collect::<Vec<_>>(),
        [address(0), address(1)]
    );
    assert_eq!(
        turn.state.spawned["Worker"],
        [Value::Bool(false), Value::Bool(false)]
    );
    for i in 0..2 {
        assert_eq!(turn.state.mailboxes[&address(i)].len(), 1);
        assert_eq!(turn.state.messages["Worker"][i].target, address(i));
        assert!(!turn.state.messages["Worker"][i].processed);
    }
    assert!(turn.state.input_processed[0]);
    assert_eq!(before, submitted(&p));
    let r = run(&s);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.claims[1].result, "REACHED");
    assert_eq!(r.claims[2].result, "VERIFIED_IN_SCOPE");
    r.witness().unwrap().validate(&s, &p).unwrap();
}

#[test]
fn choice_prefixes_do_not_consume_global_slots_or_publish_partial_allocations() {
    let s = source(
        "let first = spawn(Worker); let more = choose([Some(true), None]); send(first, ()); match more { | Some(_) -> { let second = spawn(Worker); send(second, ()); } | None -> () }",
        2,
    );
    let p = compile(&s, None).unwrap();
    let before = submitted(&p);
    let steps: Vec<_> = p
        .successors(&before)
        .unwrap()
        .into_iter()
        .filter(|s| s.action.id.starts_with("process:"))
        .collect();
    assert_eq!(steps.len(), 2);
    assert_eq!(
        steps
            .iter()
            .map(|s| s.state.spawned["Worker"].len())
            .collect::<Vec<_>>(),
        [2, 1]
    );
    for step in &steps {
        assert_eq!(step.action.spawns[0].address, address(0));
        assert_eq!(step.state.mailboxes[&address(0)].len(), 1);
    }
    assert!(!before.spawned.contains_key("Worker"));
    let r = run(&s);
    assert_eq!(r.status, Status::VerifiedInScope);
    r.witness().unwrap().validate(&s, &p).unwrap();
}

#[test]
fn each_definition_has_its_own_fresh_identity_pool() {
    let s = source("let a = spawn(Worker); let other = spawn(Other); let b = spawn(Worker); send(a, ()); send(b, ()); send(other, ());", 2)
        .replace("property \"consistent\"", "actor Other { handle_message(m: unit): unit { () } } property \"consistent\"")
        .replace("spawn_bound Worker = 2", "spawn_bound Worker = 2 spawn_bound Other = 1");
    let p = compile(&s, None).unwrap();
    let turn = p
        .successors(&submitted(&p))
        .unwrap()
        .into_iter()
        .find(|s| !s.action.spawns.is_empty())
        .unwrap();
    assert_eq!(
        turn.action
            .spawns
            .iter()
            .map(|s| s.address.clone())
            .collect::<Vec<_>>(),
        [
            address(0),
            Value::Address("Other".into(), Box::new(Value::Identity(0))),
            address(1)
        ]
    );
    assert_eq!(turn.state.spawned["Other"], [Value::Unit]);
    assert_eq!(run(&s).status, Status::VerifiedInScope);
}

#[test]
fn helper_allocations_compose_with_choices_and_replay() {
    let s = format!(
        "let create = (): Actor<Worker> {{ let w = spawn(Worker); w }} {}",
        source(
            "let a = create(); let b = create(); let w = choose([a, b]); send(w, ());",
            2
        )
    );
    let p = compile(&s, None).unwrap();
    let r = run(&s);
    // One allocated worker is deliberately left idle: creation alone is not work.
    assert_eq!(r.status, Status::Violated);
    for claim in r.claims {
        if let Some(trace) = claim.witness {
            trace.validate(&s, &p).unwrap();
            let spawns = &trace
                .actions
                .iter()
                .find(|a| !a.spawns.is_empty())
                .unwrap()
                .spawns;
            assert_eq!(spawns[0].span, spawns[1].span);
            assert_ne!(spawns[0].calls, spawns[1].calls);
        }
    }
}

#[test]
fn future_instance_temporal_clauses_are_not_vacuously_empty() {
    let good = source("let w = spawn(Worker); send(w, ());", 2);
    assert_eq!(run(&good).status, Status::VerifiedInScope);
    let broken = good.replace("Bool { true }", "Bool { false }");
    let r = run(&broken);
    assert_eq!(r.claims[2].result, "VIOLATED");
    let trace = r.claims[2].witness.as_ref().unwrap();
    assert!(trace.loop_start.is_some());
    trace
        .validate(&broken, &compile(&broken, None).unwrap())
        .unwrap();
    let unfair = good.replace("fairness { weak runtime.progress }", "");
    assert_eq!(run(&unfair).claims[2].result, "VIOLATED");
    let never_created = source("()", 2);
    let r = run(&never_created);
    assert_eq!(r.claims[1].result, "UNREACHABLE");
    assert_eq!(r.claims[2].result, "VERIFIED_IN_SCOPE");
}

#[test]
fn limits_abort_allocation_state_outbox_and_observations() {
    for s in [
        source("let w = spawn(Worker); send(w, ());", 0),
        source("let a = spawn(Worker); let b = spawn(Worker);", 1),
        source(
            "let w = spawn(Worker); send(w, ()); send(w, ()); send(w, ());",
            1,
        ),
        source(
            "let a = spawn(Worker); let b = spawn(Worker); send(a, ()); send(b, ());",
            2,
        )
        .replace("mailbox_bound = 2", "mailbox_bound = 2 message_bound = 1"),
        source("let w = spawn(Worker);", 1).replace(
            "init(): Bool { false }",
            "init(): Bool { let outside = 1; false }",
        ),
        source(
            "let a = spawn(Worker); let x = choose([Some(true), None]); match x { | Some(_) -> { let b = spawn(Worker); } | None -> () }",
            1,
        ),
    ] {
        // The initializer case is a domain escape, not an absent Int domain.
        let s = s.replace(
            "spawn_bound Worker =",
            "domain Int = [0] spawn_bound Worker =",
        );
        let p = compile(&s, None).unwrap();
        let before = submitted(&p);
        let unchanged = before.clone();
        assert!(
            p.successors(&before)
                .unwrap_err()
                .message
                .starts_with("LIMIT:"),
            "{s}"
        );
        assert_eq!(before, unchanged);
        assert!(!before.spawned.contains_key("Worker"));
        assert!(!before.input_processed[0]);
        let r = run(&s);
        assert_eq!(r.status, Status::Inconclusive, "{s}");
        assert!(!r.complete);
    }
    let zero = source("()", 0);
    assert_eq!(run(&zero).status, Status::VerifiedInScope);
}

#[test]
fn identities_do_not_require_integer_domains_and_can_be_forwarded_for_self_send() {
    let s = "type Message = Begin(Actor<Worker>) | Finish
        actor Parent { handle_message(m: unit): unit { let w = spawn(Worker); send(w, Begin(w)); } }
        actor Worker { init(): Bool { false } handle_message(s: Bool, m: Message): Bool { match m { | Begin(me) -> { send(me, Finish); false } | Finish -> true } } }
        property \"self send completes\" { forall (w in instances(Worker)) { w.created leads_to w.state == Some(true) } }
        property \"done\" { reachable (exists (w in instances(Worker)) { w.state == Some(true) }) }
        check C { spawn_bound Parent = 1 spawn_bound Worker = 1 mailbox_bound = 1 main { let parent = spawn(Parent); inputs { once send(parent, ()) } } fairness { weak runtime.progress } }";
    let r = run(s);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.claims[1].result, "REACHED");
    r.witness()
        .unwrap()
        .validate(s, &compile(s, None).unwrap())
        .unwrap();
}

#[test]
fn stateless_workers_have_no_state_field() {
    let s = source("let w = spawn(Worker); send(w, ());", 1)
        .replace(
            "init(): Bool { false } handle_message(s: Bool, m: unit): Bool { true }",
            "handle_message(m: unit): unit { () }",
        )
        .replace(
            "w.created == (w.reference != None) && w.created == (w.state != None)",
            "w.created == (w.reference != None)",
        )
        .replace("w.state == Some(true)", "w.created");
    assert_eq!(run(&s).status, Status::VerifiedInScope);
    assert!(
        compile(
            &s.replace(
                "reachable (exists (w in instances(Worker)) { w.created })",
                "reachable (exists (w in instances(Worker)) { w.state == Some(()) })"
            ),
            None
        )
        .is_err()
    );
}

#[test]
fn invalid_definition_bound_reference_and_effect_forms_are_rejected() {
    let good = source("let w = spawn(Worker); send(w, ());", 2);
    for s in [
        good.replace("actor Worker", "spawnable actor Worker"),
        good.replace("actor Worker", "actor Worker(id: Bool)"),
        good.replace("spawn_bound Worker = 2", ""),
        good.replace("spawn_bound Worker = 2", "spawn_bound Worker = 4097"),
        good.replace("spawn_bound Worker = 2", "spawn_bound Worker = \"2\""),
        good.replace("spawn_bound Worker = 2", "spawn_bound Missing = 2"),
        good.replace("spawn_bound Worker = 2", "spawn_bound Parent = 2"),
        good.replace(
            "spawn_bound Worker = 2",
            "spawn_bound Worker = 2 spawn_bound Worker = 1",
        ),
        good.replace("init(): Bool { false }", "init(key: Bool): Bool { key }"),
        good.replace("once send(parent, ())", "once send(Worker, ())"),
        good.replace("let w = spawn(Worker);", "let w = Worker;"),
        good.replace("let w = spawn(Worker);", "let w = Worker.at(true);"),
        good.replace("spawn(Worker)", "spawn(Worker, true)"),
        good.replace("spawn(Worker)", "Some(spawn(Worker))"),
        good.replace("spawn(Worker)", "choose([spawn(Worker)])"),
        good.replace(
            "let w = spawn(Worker); send(w, ());",
            "let x = instances(Worker);",
        ),
        good.replace("instances(Worker)", "instances(Parent)"),
        good.replace("w.state == Some(true)", "Worker.state"),
        good.replace("instances(Worker)", "inputs(Worker)"),
    ] {
        assert!(compile(&s, None).is_err(), "accepted {s}");
    }
    let helper = "let allocate = (): Actor<Worker> { let w = spawn(Worker); w } let indirect = (): Actor<Worker> { let w = allocate(); w } ";
    for s in [
        good.replace(
            "init(): Bool { false }",
            "init(): Bool { let w = indirect(); false }",
        ),
        good.replace("let w = spawn(Worker);", "let w = choose([indirect()]);"),
        good.replace("once send(parent, ())", "once send(parent, indirect())"),
        good.replace(
            "w.created == (w.reference != None)",
            "Some(indirect()) == w.reference",
        ),
    ] {
        assert!(
            compile(&format!("{helper}{s}"), None).is_err(),
            "accepted {s}"
        );
    }
    for declaration in [
        "type spawn = X",
        "type instances = X",
        "let spawn = (): unit { () }",
        "actor instances { handle_message(m: unit): unit { () } }",
    ] {
        assert!(compile(&format!("{declaration} {good}"), None).is_err());
    }
}

#[test]
fn potential_references_cannot_be_fabricated_through_type_domain_enumeration() {
    let s = format!("type Wrapper = Wrap(Actor<Worker>) {}", source("()", 1))
        .replace("property \"consistent\"", "property \"enumerate\" { always (forall (w in Wrapper) { w == w }) } property \"consistent\"");
    let p = compile(&s, None).unwrap();
    assert!(
        checker::check(&s, &p, &Options::default())
            .unwrap_err()
            .message
            .contains("cannot enumerate")
    );
}

#[test]
fn setup_and_handler_instances_share_the_total_address_guard() {
    // Stay below the per-function elaboration guard while filling the shared
    // address pool across setup and a handler, then try one more allocation.
    let setup = "spawn(A);".repeat(2999);
    let batch = "spawn(Worker);".repeat(1096);
    let s = format!(
        "actor A {{ handle_message(m: unit): unit {{ {batch} }} }} actor Worker {{ handle_message(m: unit): unit {{}} }} property \"safe\" {{ always true }} check C {{ spawn_bound A = 3000 spawn_bound Worker = 1097 mailbox_bound = 2 main {{ let a = spawn(A); {setup} inputs {{ once send(a, ()) once send(a, ()) }} }} }}"
    );
    let p = compile(&s, None).unwrap();
    assert_eq!(p.initial().unwrap().mailboxes.len(), 3000);
    let first = p
        .successors(&submitted(&p))
        .unwrap()
        .into_iter()
        .find(|s| s.action.id.starts_with("process:"))
        .unwrap()
        .state;
    assert_eq!(first.mailboxes.len(), 4096);
    let before = p
        .successors(&first)
        .unwrap()
        .into_iter()
        .find(|s| s.action.id == "submit:1")
        .unwrap()
        .state;
    assert!(
        p.successors(&before)
            .unwrap_err()
            .message
            .contains("total actor address capacity")
    );
    assert_eq!(before.spawned["Worker"].len(), 1096);
    assert_eq!(before.spawned["A"].len(), 3000);
}

#[test]
fn coordinator_example_and_wrong_correlation_regression() {
    let s = include_str!("../examples/spawn-workers.fml");
    let p = compile(s, None).unwrap();
    let r = run(s);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.claims[2].result, "REACHED");
    r.witness().unwrap().validate(s, &p).unwrap();
    assert_eq!(
        run(&s.replace("spawn_bound Worker = 2", "spawn_bound Worker = 1")).status,
        Status::Inconclusive
    );
    assert_eq!(
        run(&s.replace("Done(work.job)", "Done(Alpha)")).status,
        Status::Violated
    );
}

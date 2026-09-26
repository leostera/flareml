use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Value,
};

const REPLY: &str = include_str!("../examples/counter-replies.fml");

fn run(source: &str) -> checker::Report {
    let program = compile(source, None).unwrap();
    checker::check(source, &program, &Options::default()).unwrap()
}

#[test]
fn asynchronous_reply_is_a_separate_turn_after_commit() {
    let program = compile(REPLY, None).unwrap();
    let report = run(REPLY);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert!(report.assumptions.iter().any(|a| a.contains("not durable")));
    let trace = report.witness().unwrap();
    assert_eq!(trace.format_version, 5);
    assert_eq!(trace.actions.len(), 3);
    assert!(!trace.actions[0].fair); // environment is optional
    assert!(trace.actions[1].description.contains("enqueue 1 message"));
    assert!(trace.actions[2].description.contains("Counted"));
    assert_eq!(
        trace.states[2].keyed_actors["Counter"][&Value::Variant("Main".into(), vec![])],
        Value::Int(1)
    );
    trace.validate(REPLY, &program).unwrap();
    let mut old = trace.clone();
    old.format_version = 4;
    assert!(old.validate(REPLY, &program).is_err());
    let mut altered_bound = trace.clone();
    altered_bound.mailbox_bound = Some(1);
    assert!(altered_bound.validate(REPLY, &program).is_err());
    let mut corrupt = trace.clone();
    corrupt.states[2].input_submitted[0] = false;
    assert!(corrupt.validate(REPLY, &program).is_err());
    let mut duplicate = serde_json::to_value(trace).unwrap();
    let queues = duplicate["states"][0]["mailboxes"].as_array_mut().unwrap();
    queues.push(queues[0].clone());
    assert!(serde_json::from_value::<flareml::trace::Trace>(duplicate).is_err());
}

#[test]
fn failing_invariant_has_a_replayable_message_trace() {
    let source = REPLY.replace("Counter.at(Main).state == 1", "Counter.at(Main).state == 0");
    let program = compile(&source, None).unwrap();
    let report = run(&source);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert_eq!(witness.actions.len(), 3);
    witness.validate(&source, &program).unwrap();
}

#[test]
fn same_address_is_fifo_and_callback_sends_are_in_source_order() {
    let source = r#"
type Start = Start
 type Message = First | Second
actor Sender {
  handle_message(message: Start): unit {
    match message {
      | Start -> {
          send(Target, First)
          send(Target, Second);
          ()
        }
    }
  }
}
actor Target {
  init(): Int { 0 }
  handle_message(state: Int, message: Message): Int {
    match message {
      | First -> 1
      | Second -> state + 2
    }
  }
}
invariant "second cannot overtake first" { Target.state != 2 }
cover "second processed after first" { Target.state == 3 }
check C {
  semantics = "actors-v2"
  domain Int = 0..3
  mailbox_bound = 2
  inputs { once send(Sender, Start) }
  fairness { weak runtime.progress }
}
"#;
    let p = compile(source, None).unwrap();
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    let trace = report.witness().unwrap();
    assert_eq!(trace.actions.len(), 4);
    assert!(trace.actions[2].description.contains("First"));
    assert!(trace.actions[3].description.contains("Second"));
    trace.validate(source, &p).unwrap();
}

#[test]
fn two_independent_addresses_match_tiny_product_oracle() {
    // Each address has three local phases: optional input absent, queued, processed.
    // The independent product has 3x3 states and 9 stutters + 12 progress edges.
    let source = r#"
type Tick = Tick
actor A {
  init(): Bool { false }
  handle_message(state: Bool, msg: Tick): Bool { true }
}
actor B {
  init(): Bool { false }
  handle_message(state: Bool, msg: Tick): Bool { true }
}
invariant "both values are Booleans" { A.state || !A.state }
cover "both have processed" { A.state && B.state }
check C {
  semantics = "actors-v2"
  mailbox_bound = 1
  inputs { once send(A, Tick) once send(B, Tick) }
  fairness { weak runtime.progress }
}
"#;
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!((report.states, report.edges), (9, 21));
}

#[test]
fn mailbox_capacity_is_inconclusive_not_dropped() {
    let source = REPLY.replace("mailbox_bound = 2", "mailbox_bound = 1").replace(
        "once send(Counter.at(Main), Inc(Client.at(User), First))",
        "once send(Counter.at(Main), Inc(Client.at(User), First)) once send(Counter.at(Main), Inc(Client.at(User), First))",
    );
    let report = run(&source);
    assert_eq!(report.status, Status::Inconclusive);
    assert!(report.cutoff.unwrap().contains("mailbox capacity"));
}

#[test]
fn self_send_commits_after_dequeue_and_remains_finite() {
    let source = r#"
type Tick = Tick
actor Loop {
  init(): Bool { false }
  handle_message(state: Bool, msg: Tick): Bool {
    match msg {
      | Tick -> {
          send(Loop, Tick)
          !state
        }
    }
  }
}
cover "loop runs" { Loop.state }
check C {
  semantics = "actors-v2"
  mailbox_bound = 1
  inputs { once send(Loop, Tick) }
  fairness { weak runtime.progress }
}
"#;
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    let witness = report.witness().unwrap();
    assert_eq!(witness.actions.len(), 2);
    assert_eq!(witness.states.last().unwrap().mailboxes.len(), 1);
    assert_eq!(
        witness
            .states
            .last()
            .unwrap()
            .mailboxes
            .values()
            .next()
            .unwrap()
            .len(),
        1
    );
    let waiting = source.replace(
        "cover \"loop runs\" { Loop.state }",
        "property \"optional input can starve\" { eventually Loop.state }",
    );
    let p = compile(&waiting, None).unwrap();
    let report = run(&waiting);
    assert_eq!(report.status, Status::Violated);
    let trace = report.witness().unwrap();
    assert_eq!(trace.loop_start, Some(0));
    assert_eq!(trace.actions[0].id, "stutter");
    trace.validate(&waiting, &p).unwrap();
}

#[test]
fn keyed_stateless_addresses_have_independent_mailboxes() {
    let source = r#"
type Key = Left | Right
 type Tick = Tick
actor Shard(id: Key) {
  handle_message(message: Tick): unit { () }
}
invariant "typed inputs are finite" { true }
check C {
  semantics = "actors-v2"
  mailbox_bound = 1
  inputs { once send(Shard.at(Left), Tick) once send(Shard.at(Right), Tick) }
  fairness { weak runtime.progress }
}
"#;
    let p = compile(source, None).unwrap();
    assert_eq!(p.initial().unwrap().mailboxes.len(), 2);
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(report.states, 9);
}

#[test]
fn singleton_address_can_be_initialized_before_target_declaration() {
    let source = r#"
type Tick = Tick
actor Earlier {
  init(): Address<Later> { Later }
  handle_message(state: Address<Later>, msg: Tick): Address<Later> {
    send(state, msg)
    state
  }
}
actor Later {
  init(): Bool { false }
  handle_message(state: Bool, msg: Tick): Bool { true }
}
cover "later receives" { Later.state }
check C {
  semantics = "actors-v2"
  mailbox_bound = 1
  inputs { once send(Earlier, Tick) }
  fairness { weak runtime.progress }
}
"#;
    let p = compile(source, None).unwrap();
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(report.witness().unwrap().actions.len(), 3);
    report.witness().unwrap().validate(source, &p).unwrap();
}

#[test]
fn transitive_send_helper_is_staged_and_init_cannot_send() {
    let source = REPLY.replace(
        "actor Counter(id: CounterId) {",
        "let notify = (address: Address<Client>, message: ClientMessage): unit { send(address, message) }\nactor Counter(id: CounterId) {",
    ).replace(
        "send(reply_to, Counted(request_id, next))",
        "notify(reply_to, Counted(request_id, next))",
    );
    assert_eq!(run(&source).status, Status::VerifiedInScope);
    let bad = source.replace(
        "init(id: CounterId): Int { 0 }",
        "init(id: CounterId): Int { notify(Client.at(User), Counted(First, 0)) 0 }",
    );
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("init must be pure")
    );
}

#[test]
fn profile_and_message_type_fail_closed() {
    for source in [
        REPLY.replace("actors-v2", "actors-v1"),
        REPLY.replace("Inc(Client.at(User), First)", "Counted(First, 1)"),
        REPLY.replace(
            "send(reply_to, Counted(request_id, next))",
            "send(reply_to, Inc(reply_to, request_id))",
        ),
        REPLY.replace("mailbox_bound = 2", ""),
        REPLY
            .replace("| Counted(_, _) -> Observed", "| Counted(_, _) -> Waiting")
            .replace(
                "handle_message(state: ClientState, message: ClientMessage): ClientState",
                "handle_message(state: Int, message: ClientMessage): ClientState",
            ),
    ] {
        assert!(
            compile(&source, None).is_err(),
            "unexpectedly accepted {source}"
        );
    }
    assert!(
        compile(
            &REPLY.replace(
                "handle_message(state: Int, message: CounterMessage): Int",
                "handle_message(state: Int, message: CounterMessage): unit"
            ),
            None
        )
        .is_err()
    );
    assert!(
        compile(
            &REPLY.replace(
                "send(reply_to, Counted(request_id, next))",
                "call(Counter.handle_message, next)"
            ),
            None
        )
        .is_err()
    );
}

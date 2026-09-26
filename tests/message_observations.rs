use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::{State, Value},
    trace::Trace,
};

const REPLIES: &str = include_str!("../examples/counter-replies.fml");
fn run(source: &str) -> checker::Report {
    checker::check(source, &compile(source, None).unwrap(), &Options::default()).unwrap()
}

#[test]
fn dynamically_generated_messages_are_not_an_empty_initial_quantifier() {
    let report = run(REPLIES);
    assert_eq!(report.status, Status::VerifiedInScope);
    let claim = report
        .claims
        .iter()
        .find(|c| c.name == "generated replies are eventually processed")
        .unwrap();
    // Spare slots may be unused, but the property cannot be vacuous due to no
    // messages existing when temporal clauses are expanded at initialization.
    assert!(claim.note.as_ref().is_none_or(|n| !n.contains("empty")));
    let source = REPLIES.replace("fairness { weak runtime.progress }", "");
    let program = compile(&source, None).unwrap();
    let report = checker::check(
        &source,
        &program,
        &Options {
            property: Some(claim.name.clone()),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert!(witness.loop_start.is_some());
    assert!(
        witness.states.last().unwrap().messages["Client"]
            .iter()
            .any(|m| !m.processed)
    );
    witness.validate(&source, &program).unwrap();
}

const TWINS: &str = r#"
type Msg = Ping
actor Sink { handle_message(msg: Msg): unit { () } }
property "inputs processed only after submission" {
  always (forall (i in inputs(Sink)) { i.processed implies i.submitted })
}
property "observations are typed" {
  always (forall (m in messages(Sink)) {
    m.sent implies m.payload == Some(Ping) && (exists (sink in instances(Sink)) { sink.reference == m.target }) && m.external
  })
}
property "every external input progresses" {
  forall (i in inputs(Sink)) { i.submitted leads_to i.processed }
}
property "every envelope progresses" {
  forall (m in messages(Sink)) { m.sent leads_to m.processed }
}
property "two sends of equal payload are distinct" {
  reachable (forall (m in messages(Sink)) { m.sent && m.processed })
}
check C {
  mailbox_bound = 2
  message_bound = 2
  spawn_bound Sink = 1
  main { let sink = spawn(Sink); inputs { once send(sink, Ping) once send(sink, Ping) } }
  fairness { weak runtime.progress }
}
"#;

#[test]
fn identical_payloads_keep_separate_input_and_message_identities() {
    let p = compile(TWINS, None).unwrap();
    let report = run(TWINS);
    assert_eq!(report.status, Status::VerifiedInScope);
    let witness = report.witness().unwrap();
    let end = witness.states.last().unwrap();
    assert_eq!(end.input_processed, vec![true, true]);
    assert_eq!(end.messages["Sink"].len(), 2);
    witness.validate(TWINS, &p).unwrap();
    let mut bad = witness.clone();
    bad.states
        .last_mut()
        .unwrap()
        .messages
        .get_mut("Sink")
        .unwrap()[1]
        .processed = false;
    assert!(bad.validate(TWINS, &p).is_err());
    let mut bad = witness.clone();
    bad.message_bound = Some(3);
    assert!(bad.validate(TWINS, &p).is_err());
    let roundtrip: Trace = serde_json::from_str(&serde_json::to_string(witness).unwrap()).unwrap();
    roundtrip.validate(TWINS, &p).unwrap();
}

#[test]
fn two_senders_can_enqueue_in_either_order_without_merging_envelopes() {
    let source = r#"
type Msg = Start | First | Second
actor A { handle_message(sink: Actor<Sink>): unit { send(sink, First) } }
actor B { handle_message(sink: Actor<Sink>): unit { send(sink, Second) } }
actor Sink {
  init(): Msg { Start }
  handle_message(state: Msg, msg: Msg): Msg { msg }
}
property "generated work completes" { forall (m in messages(Sink)) { m.sent leads_to m.processed } }
property "A then B" { reachable ((exists (sink in instances(Sink)) { sink.state == Some(Second) }) && forall (m in messages(Sink)) { m.processed }) }
property "B then A" { reachable ((exists (sink in instances(Sink)) { sink.state == Some(First) }) && forall (m in messages(Sink)) { m.processed }) }
check C {
  mailbox_bound = 2 message_bound = 2
  spawn_bound A = 1 spawn_bound B = 1 spawn_bound Sink = 1
  main { let a = spawn(A); let b = spawn(B); let sink = spawn(Sink); inputs { once send(a, sink) once send(b, sink) } }
  fairness { weak runtime.progress }
}
"#;
    let p = compile(source, None).unwrap();
    let report = run(source);
    assert_eq!(report.status, Status::VerifiedInScope);
    for c in report
        .claims
        .iter()
        .filter(|c| c.kind == flareml::syntax::ClaimKind::Cover)
    {
        assert_eq!(c.result, "REACHED");
        c.witness.as_ref().unwrap().validate(source, &p).unwrap();
    }
}

#[test]
fn observation_capacity_is_a_cutoff_not_slot_reuse() {
    let source = TWINS.replace("message_bound = 2", "message_bound = 1");
    let report = run(&source);
    assert_eq!(report.status, Status::Inconclusive);
    assert!(
        report
            .cutoff
            .unwrap()
            .contains("message observation capacity")
    );
}

#[test]
fn observation_slots_never_recycle_even_in_self_sending_cycles() {
    let source = r#"
type Msg = Ping
actor Loop { handle_message(me: Actor<Loop>): unit { send(me, me) } }
property "all sends finish" { forall (m in messages(Loop)) { m.sent leads_to m.processed } }
check C {
  mailbox_bound = 1
  message_bound = 3
  spawn_bound Loop = 1
  main { let participant = spawn(Loop); inputs { once send(participant, participant) } }
  fairness { weak runtime.progress }
}
"#;
    let report = run(source);
    assert_eq!(report.status, Status::Inconclusive);
    assert!(
        report
            .cutoff
            .unwrap()
            .contains("message observation capacity")
    );
    let unobserved = source.replace("message_bound = 3", "").replace(
        "property \"all sends finish\" { forall (m in messages(Loop)) { m.sent leads_to m.processed } }",
        "property \"external send finishes\" { forall (i in inputs(Loop)) { i.submitted leads_to i.processed } }",
    );
    assert_eq!(run(&unobserved).status, Status::VerifiedInScope);
}

#[test]
fn inspectors_cannot_escape_into_behavior_or_use_unbounded_history() {
    for source in [
        TWINS.replace("message_bound = 2", ""),
        TWINS.replace("messages(Sink)", "messages(Sink.at(()))"),
        TWINS.replace(
            "handle_message(msg: Msg): unit { () }",
            "handle_message(msg: Msg): unit { let view = inputs(Sink); () }",
        ),
        TWINS.replace("m.payload == Some(Ping)", "m.payload == Ping"),
        TWINS.replace("m.external", "m.response"),
    ] {
        assert!(compile(&source, None).is_err(), "{source}");
    }
    let helper = format!(
        "let inspect = (x: unit): Bool {{ exists (m in messages(Sink)) {{ m.sent }} }}\n{}",
        TWINS.replace(
            "handle_message(msg: Msg): unit { () }",
            "handle_message(msg: Msg): unit { let result = inspect(()); () }"
        )
    );
    assert!(compile(&helper, None).is_err());
}

#[test]
fn unsent_slot_defaults_and_input_payload_are_explicit() {
    let p = compile(TWINS, None).unwrap();
    let s = p.initial().unwrap();
    assert!(s.messages.is_empty());
    let source = TWINS.replace("m.sent implies m.payload == Some(Ping) && (exists (sink in instances(Sink)) { sink.reference == m.target }) && m.external", "(!m.sent implies m.payload == None && m.target == None && !m.processed && !m.external) && (m.processed implies m.sent)").replace(
        "i.processed implies i.submitted", "i.payload == Ping && (exists (sink in instances(Sink)) { sink.reference == Some(i.target) }) && (i.processed implies i.submitted)");
    assert_eq!(run(&source).status, Status::VerifiedInScope);
}

#[test]
fn missing_reply_fixture_exposes_protocol_failure_not_scheduler_starvation() {
    let source = include_str!("../examples/missing-reply.fml");
    let p = compile(source, None).unwrap();
    let report = run(source);
    assert_eq!(report.status, Status::Violated);
    assert_eq!(report.claims[1].result, "VERIFIED_IN_SCOPE");
    let trace = report.witness().unwrap();
    assert!(trace.weak_progress);
    assert!(trace.loop_start.is_some());
    assert!(
        trace
            .states
            .last()
            .unwrap()
            .mailboxes
            .values()
            .all(Vec::is_empty)
    );
    trace.validate(source, &p).unwrap();
    let fixed = source
        .replace(
            "| Inc(reply_to) -> state + 1",
            "| Inc(reply_to) -> { let next = state + 1; send(reply_to, Counted(next)); next }",
        )
        .replace("message_bound = 1", "message_bound = 2");
    assert_eq!(run(&fixed).status, Status::VerifiedInScope);
}

#[test]
fn input_completion_does_not_mean_reply_or_followup_completion() {
    let p = compile(REPLIES, None).unwrap();
    let initial = p.initial().unwrap();
    let submitted = p
        .successors(&initial)
        .unwrap()
        .into_iter()
        .find(|s| s.action.id == "submit:0")
        .unwrap()
        .state;
    let committed = p
        .successors(&submitted)
        .unwrap()
        .into_iter()
        .find(|s| s.action.id.starts_with("process:"))
        .unwrap()
        .state;
    assert!(committed.input_processed[0]);
    assert!(!committed.messages["Client"][0].processed);
    assert!(!committed.messages["Client"][0].external);
}

#[test]
fn busy_self_sender_cannot_starve_a_different_enabled_mailbox_under_fairness() {
    let source = r#"
type Msg = Ping
actor Busy { handle_message(me: Actor<Busy>): unit { send(me, me) } }
actor Victim { handle_message(msg: Msg): unit { () } }
property "victim progresses" { forall (i in inputs(Victim)) { i.submitted leads_to i.processed } }
check C {
  mailbox_bound = 1
  spawn_bound Busy = 1 spawn_bound Victim = 1
  main { let busy = spawn(Busy); let victim = spawn(Victim); inputs { once send(busy, busy) once send(victim, Ping) } }
  fairness { weak runtime.progress }
}
"#;
    assert_eq!(run(source).status, Status::VerifiedInScope);
    let unfair = source.replace("fairness { weak runtime.progress }", "");
    let report = run(&unfair);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    witness
        .validate(&unfair, &compile(&unfair, None).unwrap())
        .unwrap();
    // Forging the declared fairness cannot turn an unfair starvation lasso into evidence.
    let mut forged = witness.clone();
    forged.source_hash = flareml::trace::source_hash(source);
    forged.weak_progress = true;
    for action in &mut forged.actions {
        action.fair = action.id.starts_with("process:");
    }
    assert!(
        forged
            .validate(source, &compile(source, None).unwrap())
            .unwrap_err()
            .message
            .contains("fairness")
    );
}

#[test]
fn waiting_for_a_missing_reply_fails_even_with_fairness() {
    let source = r#"
type Msg = Start
actor Client {
  init(): Bool { false }
  handle_message(state: Bool, msg: Msg): Bool { true }
}
property "missing reply" { forall (client in instances(Client)) { client.state == Some(true) leads_to client.state == Some(false) } }
check C {
  mailbox_bound = 1
  spawn_bound Client = 1
  main { let client = spawn(Client); inputs { once send(client, Start) } }
  fairness { weak runtime.progress }
}
"#;
    let report = run(source);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert!(
        witness
            .states
            .last()
            .unwrap()
            .mailboxes
            .values()
            .all(Vec::is_empty)
    );
    witness
        .validate(source, &compile(source, None).unwrap())
        .unwrap();
}

#[test]
fn message_bounds_and_provenance_are_validated_in_replay() {
    let p = compile(TWINS, None).unwrap();
    let report = run(TWINS);
    let trace = report.witness().unwrap();
    let mut changed = trace.clone();
    let first_queued = changed
        .states
        .iter_mut()
        .find(|s| s.mailboxes.values().any(|q| !q.is_empty()))
        .unwrap();
    first_queued
        .mailboxes
        .values_mut()
        .find(|q| !q.is_empty())
        .unwrap()[0]
        .input = None;
    assert!(changed.validate(TWINS, &p).is_err());
    assert!(State::default().messages.is_empty());
    assert_eq!(
        Value::Message("Sink".into(), 0).to_string(),
        "Sink message #0"
    );
}

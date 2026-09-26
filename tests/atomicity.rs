use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Value,
};
const SOURCE: &str = r#"
type Message = Start(Actor<Target>) | First | Second
actor Sender {
  init(): Bool { false }
  handle_message(state: Bool, message: Message): Bool {
    match message {
      | Start(target) -> {
          send(target, First);
          send(target, Second);
          true
        }
      | _ -> state
    }
  }
}
actor Target { handle_message(message: Message): unit { () } }
property "commit" { reachable (exists (sender in instances(Sender)) { sender.state == Some(true) }) }
check C { spawn_bound Sender = 1 spawn_bound Target = 1 mailbox_bound = 1 main { let sender = spawn(Sender); let target = spawn(Target); inputs { once send(sender, Start(target)) } } }
"#;
#[test]
fn overflowing_second_send_does_not_partly_commit_the_turn() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace("mailbox_bound = 1", "mailbox_bound = 2 message_bound = 1"),
    ] {
        let p = compile(&source, None).unwrap();
        let submitted = p
            .successors(&p.initial().unwrap())
            .unwrap()
            .into_iter()
            .find(|s| s.action.id == "submit:0")
            .unwrap()
            .state;
        let before = submitted.clone();
        assert!(
            p.successors(&submitted)
                .unwrap_err()
                .message
                .starts_with("LIMIT:")
        );
        assert_eq!(submitted, before);
        assert_eq!(submitted.spawned["Sender"][0], Value::Bool(false));
        assert!(!submitted.input_processed[0]);
        assert!(
            submitted.mailboxes[&Value::Address("Target".into(), Box::new(Value::Identity(0)))]
                .is_empty()
        );
        assert!(!submitted.messages.contains_key("Target"));
        let report = checker::check(&source, &p, &Options::default()).unwrap();
        assert_eq!(report.status, Status::Inconclusive);
        assert!(report.witness().is_none());
    }
}
#[test]
fn successful_commit_publishes_all_sends_in_order_and_completes_input() {
    let source = SOURCE.replace("mailbox_bound = 1", "mailbox_bound = 2 message_bound = 2");
    let p = compile(&source, None).unwrap();
    let r = checker::check(&source, &p, &Options::default()).unwrap();
    let trace = r.witness().unwrap();
    let s = trace.states.last().unwrap();
    assert_eq!(s.spawned["Sender"][0], Value::Bool(true));
    assert!(s.input_processed[0]);
    assert_eq!(
        s.messages["Target"]
            .iter()
            .map(|m| m.payload.clone())
            .collect::<Vec<_>>(),
        [
            Value::Variant("First".into(), vec![]),
            Value::Variant("Second".into(), vec![])
        ]
    );
    assert!(s.messages["Target"].iter().all(|m| !m.processed));
    trace.validate(&source, &p).unwrap();
}
#[test]
fn self_send_observes_committed_next_state_not_old_snapshot() {
    let source = r#"
type Message = Start(Actor<A>) | Finish
actor A {
  init(): Int { 0 }
  handle_message(state: Int, msg: Message): Int {
    match msg {
      | Start(me) -> { send(me, Finish); state + 1 }
      | Finish -> state + 1
    }
  }
}
property "completed" { reachable (exists (a in instances(A)) { a.state == Some(2) }) }
check C { domain Int = 0..2 spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); inputs { once send(a, Start(a)) } } fairness { weak runtime.progress } }
"#;
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Options::default()).unwrap();
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.witness().unwrap().actions.len(), 3);
    r.witness().unwrap().validate(source, &p).unwrap();
}
#[test]
fn nested_send_helpers_preserve_source_order_and_branch_scoping() {
    let source = SOURCE.replace("actor Sender {", "let last = (target: Actor<Target>): unit { send(target, Second) }\nlet notify = (target: Actor<Target>): unit { send(target, First); last(target) }\nactor Sender {")
        .replace("send(target, First);\n          send(target, Second);", "notify(target);")
        .replace("mailbox_bound = 1", "mailbox_bound = 2 message_bound = 2");
    let p = compile(&source, None).unwrap();
    let r = checker::check(&source, &p, &Options::default()).unwrap();
    let records = &r.witness().unwrap().states.last().unwrap().messages["Target"];
    assert_eq!(records[0].payload, Value::Variant("First".into(), vec![]));
    assert_eq!(records[1].payload, Value::Variant("Second".into(), vec![]));
}

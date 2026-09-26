use flareml::{
    checker::{self, Options},
    compile,
    explorer::Session,
    semantics::Value,
};
use serde_json::json;
fn witness(source: &str) -> (flareml::model::Program, flareml::trace::Trace) {
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Options::default()).unwrap();
    (p, r.witness().unwrap().clone())
}
#[test]
fn setup_projection_is_lossless_and_source_is_unicode_safe() {
    let source = "// λ 漢字\nactor A { init(): Int { 9223372036854775807 } handle_message(s: Int, m: unit): Int { 0 } } property \"</script><img src=x>\" { reachable (exists (a in instances(A)) { a.state == Some(0) }) } check C { domain Int = [0,9223372036854775807] spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); send(a, ()); } fairness { weak runtime.progress } }";
    let (p, t) = witness(source);
    let session = Session::validated(source.into(), t, &p).unwrap();
    let first = session.snapshot(0).unwrap();
    assert_eq!(first["actors"][0]["state"]["Int"], "9223372036854775807");
    assert_eq!(
        first["event"]["spawns"][0]["initial"]["Int"],
        "9223372036854775807"
    );
    assert_eq!(first["event"]["sends"][0]["source"]["text"], "send(a, ())");
    assert_eq!(first["event"]["sends"][0]["source"]["line"], 2);
    assert_eq!(first["actors"][0]["mailbox"][0]["id"], "envelope:0:0");
    assert_eq!(
        session.snapshot(1).unwrap()["event"]["consumed"]["id"],
        "envelope:0:0"
    );
    let fixture = json!({"metadata":session.metadata(),"summaries":session.summaries(0,""),"snapshots":[first,session.snapshot(1).unwrap()]});
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("explorer/tests/fixtures/lossless.json");
    if std::env::var_os("FML_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&fixture).unwrap()),
        )
        .unwrap();
    }
    let saved: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path)
            .expect("generate with FML_UPDATE_FIXTURES=1 cargo test --test explorer"),
    )
    .unwrap();
    assert_eq!(saved, fixture, "viewer schema fixture drift");
}
#[test]
fn all_examples_project_exact_queues_and_structured_events() {
    for name in [
        "explicit-startup",
        "spawn-workers",
        "spawn-choice-workers",
        "missing-reply",
        "faulty-link-loss",
        "faulty-link-duplicate-bug",
        "lost-update",
    ] {
        let source = std::fs::read_to_string(format!("examples/{name}.fml")).unwrap();
        let (p, t) = witness(&source);
        let session = Session::validated(source.clone(), t.clone(), &p).unwrap();
        for (i, state) in t.states.iter().enumerate() {
            let frame = session.snapshot(i).unwrap();
            assert_eq!(
                frame["actors"].as_array().unwrap().len(),
                state.mailboxes.len()
            );
            let mut ids = std::collections::BTreeSet::new();
            for actor in frame["actors"].as_array().unwrap() {
                let address = Value::Address(
                    actor["name"].as_str().unwrap().into(),
                    Box::new(Value::Identity(actor["slot"].as_u64().unwrap() as usize)),
                );
                let queue = actor["mailbox"].as_array().unwrap();
                assert_eq!(queue.len(), state.mailboxes[&address].len());
                for (message, envelope) in queue.iter().zip(&state.mailboxes[&address]) {
                    assert!(ids.insert(message["id"].as_str().unwrap()));
                    assert_eq!(
                        message["payload"],
                        flareml::explorer::lossless(
                            serde_json::to_value(&envelope.payload).unwrap()
                        )
                    );
                }
            }
        }
        assert!(session.snapshot(t.states.len()).is_none());
        assert_eq!(session.summaries(100000, "process")["items"], json!([]));
        let mut bad = t.clone();
        bad.states[0].mailboxes.clear();
        assert!(Session::validated(source, bad, &p).is_err());
    }
}
#[test]
fn identical_payloads_and_self_sends_keep_trace_local_identity_without_history() {
    let source = "actor A { init(): Bool { false } handle_message(s: Bool, me: Actor<A>): Bool { send(me, me); true } } property \"done\" { reachable (exists (a in instances(A)) { a.state == Some(true) }) } check C { spawn_bound A = 1 mailbox_bound = 2 main { let a = spawn(A); send(a, a); send(a, a); } }";
    let (p, t) = witness(source);
    let session = Session::validated(source.into(), t, &p).unwrap();
    let a = session.snapshot(0).unwrap();
    let b = session.snapshot(1).unwrap();
    assert_eq!(
        a["actors"][0]["mailbox"][0]["payload"],
        a["actors"][0]["mailbox"][1]["payload"]
    );
    assert_eq!(a["flows"], json!([]));
    assert_eq!(
        b["flows"],
        json!([{"source":"actor:A:0","target":"actor:A:0","count":1}])
    );
    assert_eq!(b["event"]["consumed"]["id"], "envelope:0:0");
    assert_eq!(b["actors"][0]["mailbox"][0]["id"], "envelope:0:1");
    assert_eq!(b["actors"][0]["mailbox"][1]["id"], "envelope:1:0");
}
#[test]
fn source_order_across_targets_is_captured_not_guessed_from_sorted_queues() {
    let source = "type Targets = Targets { a: Actor<Sink>, b: Actor<Sink> } actor Sender { handle_message(m: Targets): unit { send(m.b, ()); send(m.a, ()); send(m.b, ()); } } actor Sink { handle_message(m: unit): unit {} } property \"sent\" { reachable (exists (i in inputs(Sender)) { i.processed }) } check C { spawn_bound Sender = 1 spawn_bound Sink = 2 mailbox_bound = 2 main { let sender = spawn(Sender); let a = spawn(Sink); let b = spawn(Sink); inputs { once send(sender, Targets { a: a, b: b }) } } }";
    let (p, t) = witness(source);
    let session = Session::validated(source.into(), t, &p).unwrap();
    let frame = session.snapshot(2).unwrap();
    let targets: Vec<_> = frame["event"]["sends"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, ["actor:Sink:1", "actor:Sink:0", "actor:Sink:1"]);
    assert_eq!(session.snapshot(0).unwrap()["flows"], json!([]));
    assert_eq!(session.snapshot(1).unwrap()["flows"], json!([]));
    assert_eq!(
        frame["flows"],
        json!([
            {"source":"actor:Sender:0","target":"actor:Sink:0","count":1},
            {"source":"actor:Sender:0","target":"actor:Sink:1","count":2}
        ])
    );
    assert_eq!(
        frame["actors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["stateful"] == false)
            .count(),
        3
    );
}
#[test]
fn historical_routes_go_idle_and_rewinding_does_not_leak_future_sends() {
    let source = "actor Sender { handle_message(target: Actor<Sink>): unit { send(target, ()); } } actor Sink { init(): Bool { false } handle_message(s: Bool, m: unit): Bool { true } } property \"done\" { reachable (exists (a in instances(Sink)) { a.state == Some(true) }) } check C { spawn_bound Sender = 1 spawn_bound Sink = 1 mailbox_bound = 1 main { let sink = spawn(Sink); let sender = spawn(Sender); send(sender, sink); } }";
    let (p, t) = witness(source);
    let session = Session::validated(source.into(), t, &p).unwrap();
    let initial = session.snapshot(0).unwrap();
    assert_eq!(initial["actors"][0]["name"], "Sink");
    assert_eq!(initial["actors"][0]["spawn_order"], 0);
    assert_eq!(initial["actors"][1]["name"], "Sender");
    assert_eq!(initial["actors"][1]["spawn_order"], 1);
    assert_eq!(
        session.snapshot(2).unwrap()["flows"],
        json!([{"source":"actor:Sender:0","target":"actor:Sink:0","count":0}])
    );
    assert_eq!(session.snapshot(1).unwrap()["flows"][0]["count"], 1);
    assert_eq!(session.snapshot(0).unwrap()["flows"], json!([]));
}
#[test]
fn zero_step_witness_and_stutter_lasso_have_real_snapshots() {
    for formula in ["reachable true", "eventually false"] {
        let source =
            format!("property \"p\" {{ {formula} }} check C {{ mailbox_bound = 1 main {{}} }}");
        let (p, t) = witness(&source);
        let session = Session::validated(source, t.clone(), &p).unwrap();
        assert_eq!(session.snapshot(0).unwrap()["event"]["kind"], "setup");
        if formula == "reachable true" {
            assert_eq!(t.states.len(), 1);
        } else {
            assert_eq!(session.metadata()["loop_start"], 0);
            assert_eq!(session.snapshot(1).unwrap()["event"]["kind"], "stutter");
        }
    }
}

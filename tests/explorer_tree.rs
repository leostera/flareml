use flareml::{
    checker::{self, Options},
    compile,
    explorer::Session,
};

#[test]
fn saved_choices_share_only_their_exact_prefix_and_keep_all_endpoints() {
    let source = "type Pick = Zero | One | Two actor A { init(): Pick { Zero } handle_message(s: Pick, m: unit): Pick { let pick = choose([One, Two]); pick } } property \"one\" { reachable (exists (a in instances(A)) { a.state == Some(One) }) } property \"two\" { reachable (exists (a in instances(A)) { a.state == Some(Two) }) } check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); send(a, ()); } }";
    let program = compile(source, None).unwrap();
    let report = checker::check(source, &program, &Options::default()).unwrap();
    let traces: Vec<_> = report
        .claims
        .iter()
        .filter_map(|c| c.witness.clone())
        .collect();
    assert_eq!(traces.len(), 2);
    let mut session = Session::validated(source.into(), traces[0].clone(), &program).unwrap();
    session.add_witness(Session::validated(source.into(), traces[1].clone(), &program).unwrap());
    session.add_witness(Session::validated(source.into(), traces[0].clone(), &program).unwrap());
    let tree = session.tree();
    assert_eq!(tree["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(tree["traces"][0]["path"], tree["traces"][2]["path"]);
    assert_eq!(tree["traces"][0]["path"][0], tree["traces"][1]["path"][0]);
    assert_ne!(tree["traces"][0]["path"][1], tree["traces"][1]["path"][1]);
    assert_ne!(
        tree["nodes"][1]["choices"][0]["candidate"],
        tree["nodes"][2]["choices"][0]["candidate"]
    );
    assert_eq!(tree["nodes"][1]["ends"], serde_json::json!([0, 2]));
    assert_eq!(tree["traces"][0]["violation"], false);
    assert!(session.execution(3).is_none());
    assert_ne!(
        session.execution(0).unwrap().snapshot(1),
        session.execution(1).unwrap().snapshot(1)
    );
}

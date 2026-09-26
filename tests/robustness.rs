use flareml::compile;
use proptest::prelude::*;
const FIXED: &str = include_str!("../examples/counter-replies.fml");
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn arbitrary_source_does_not_panic(source in ".{0,2000}") { let _ = compile(&source, None); }
    #[test]
    fn arbitrary_trace_json_does_not_panic(source in ".{0,2000}") { let _ = serde_json::from_str::<flareml::trace::Trace>(&source); }
    #[test]
    fn mutated_valid_source_does_not_panic(index in 0usize..4096, insertion in ".{0,32}") {
        let mut source = FIXED.to_owned();
        let i = index % source.len(); // fixture is ASCII
        source.insert_str(i, &insertion);
        let _ = compile(&source, None);
    }
}
#[test]
fn nested_matches_and_expressions_hit_parser_guard() {
    let mut body = "()".to_owned();
    for _ in 0..150 {
        body = format!("match msg {{ | _ -> {body} }}");
    }
    let source = format!(
        "actor API {{ handle_message(msg: Bool): unit {{ {body} }} }} property \"ok\" {{ always true }} check C {{ spawn_bound API = 0 mailbox_bound = 1 main {{}} }}"
    );
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("nesting")
    );
    let source = format!(
        "property \"deep\" {{ always {}true{} }} check C {{ mailbox_bound = 1 main {{}} }}",
        "(".repeat(150),
        ")".repeat(150)
    );
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("nesting")
    );
}
#[test]
fn duplicate_check_and_property_names_are_rejected() {
    assert!(
        compile(
            &format!("{FIXED}\ncheck OneIncrement {{ mailbox_bound = 1 main {{}} }}"),
            Some("OneIncrement")
        )
        .is_err()
    );
    assert!(
        compile(
            &format!("{FIXED}\nproperty \"client receives reply\" {{ always true }}"),
            None
        )
        .is_err()
    );
}
#[test]
fn model_types_cannot_be_cyclic_aliases() {
    assert!(
        compile(&format!("type X = Y\ntype Y = X\n{FIXED}"), None)
            .unwrap_err()
            .message
            .contains("cyclic")
    );
}
#[test]
fn same_named_constructor_is_not_silently_an_empty_type_domain() {
    let source = "type Tick = Tick property \"ticks\" { always (forall (t in Tick) { t == Tick }) } check C { mailbox_bound = 1 main {} }";
    let p = compile(source, None).unwrap();
    let report = flareml::checker::check(source, &p, &Default::default()).unwrap();
    assert_eq!(report.status, flareml::checker::Status::VerifiedInScope);
    assert_eq!(p.domain("Tick", 0).unwrap().len(), 1);
}
#[test]
fn domains_cannot_recursively_execute_helpers_or_be_empty() {
    for domain in ["[]", "[id(0)]", "[0 + 1]"] {
        let source = format!(
            "let id = (x: Int): Int {{ x }} property \"ok\" {{ always true }} check C {{ mailbox_bound = 1 domain Int = {domain} main {{}} }}"
        );
        assert!(compile(&source, None).is_err());
    }
}
#[test]
fn trace_unknown_fields_are_rejected_at_each_boundary() {
    let p = compile(FIXED, None).unwrap();
    let r = flareml::checker::check(FIXED, &p, &Default::default()).unwrap();
    let trace = serde_json::to_value(r.witness().unwrap()).unwrap();
    for path in [0, 1, 2] {
        let mut bad = trace.clone();
        match path {
            0 => bad["semantics"] = "actors-v1".into(),
            1 => bad["states"][0]["frames"] = serde_json::json!([]),
            _ => bad["actions"][0]["unknown"] = true.into(),
        }
        assert!(serde_json::from_value::<flareml::trace::Trace>(bad).is_err());
    }
}

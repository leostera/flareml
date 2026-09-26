use flareml::{
    checker::{self, Options, Status},
    compile,
};
fn model(extra: &str) -> String {
    format!("{extra} property \"ok\" {{ always true }} check C {{ mailbox_bound = 1 }}")
}
#[test]
fn empty_payload_types_are_unified_across_collection_elements() {
    let invalid = "property \"typed\" { always (forall (x in [None, Some(true), Some(1)]) { true }) } check C { mailbox_bound = 1 }";
    assert!(
        compile(invalid, None)
            .unwrap_err()
            .message
            .contains("single compatible type")
    );
    assert!(compile(&invalid.replace("Some(1)", "Some(false)"), None).is_ok());
}
#[test]
fn record_patterns_do_not_reach_an_unimplemented_runtime_branch() {
    let source = model(
        "type Input = Input { valid: Bool } actor API { handle_message(request: Input): unit { match request { | Input -> () } } }",
    );
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("record constructor patterns")
    );
}
#[test]
fn result_wildcards_and_ignored_bindings_cannot_silently_discard_errors() {
    for body in [
        "let result = outcome(); match result { | _ -> () }",
        "let result = outcome(); ()",
        "match msg { | _ -> outcome() } ()",
    ] {
        let source = model(&format!(
            "let outcome = (): Result<Bool, Bool> {{ Ok(true) }} actor API {{ handle_message(msg: unit): unit {{ {body} }} }}"
        ));
        assert!(compile(&source, None).is_err());
    }
}
#[test]
fn result_protocols_can_be_matched_and_enumerated() {
    let source = r#"
type Outcome = Outcome(Result<Bool, Bool>)
let unwrap = (result: Result<Bool, Bool>): Bool { match result { | Ok(x) -> x | Err(_) -> false } }
actor API { init(): Bool { false } handle_message(state: Bool, msg: Result<Bool, Bool>): Bool { unwrap(msg) } }
property "outcomes are finite" { always (forall (r in Outcome) { r == r }) }
property "success" { reachable API.state }
check C { mailbox_bound = 1 inputs { once send(API, Ok(true)) } }
"#;
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Options::default()).unwrap();
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.claims[1].result, "REACHED");
}
#[test]
fn tail_match_requires_every_branch_to_return_expected_type() {
    for body in [
        "match msg { | Some(x) -> true | None -> () }",
        "match msg { | Some(x) -> true }",
        "match msg { | Some(x) -> true | None -> let x = false }",
    ] {
        assert!(compile(&model(&format!("actor API {{ init(): Bool {{ false }} handle_message(state: Bool, msg: Option<Bool>): Bool {{ {body} }} }}")), None).is_err());
    }
}
#[test]
fn globals_and_locals_cannot_be_shadowed() {
    for extra in [
        "let f = (send: Bool): Bool { send }",
        "let f = (x: Bool, x: Bool): Bool { x }",
        "let f = (x: Bool): Bool { let x = false; x }",
        "type Msg = X | Y actor A { handle_message(msg: Msg): unit { match msg { | X -> () | Y -> () } } } let f = (A: Bool): Bool { A }",
    ] {
        assert!(compile(&model(extra), None).is_err());
    }
    assert!(compile("actor A { handle_message(msg: Bool): unit { () } } property \"p\" { always (forall (A in Bool) { A }) } check C { mailbox_bound = 1 }", None).is_err());
}

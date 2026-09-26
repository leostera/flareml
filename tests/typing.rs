use flareml::{
    checker::{self, Options, Status},
    compile,
};
fn model(extra: &str) -> String {
    let bounds = if extra.contains("actor API") {
        "spawn_bound API = 0"
    } else if extra.contains("actor A ") {
        "spawn_bound A = 0"
    } else {
        ""
    };
    format!(
        "{extra} property \"ok\" {{ always true }} check C {{ {bounds} mailbox_bound = 1 main {{}} }}"
    )
}
#[test]
fn empty_payload_types_are_unified_across_collection_elements() {
    let invalid = "property \"typed\" { always (forall (x in [None, Some(true), Some(1)]) { true }) } check C { mailbox_bound = 1 main {} }";
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
        "match msg { | _ -> outcome() }; ()",
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
property "success" { reachable (exists (api in instances(API)) { api.state == Some(true) }) }
check C { spawn_bound API = 1 mailbox_bound = 1 main { let api = spawn(API); inputs { once send(api, Ok(true)) } } }
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
        "match msg { | Some(x) -> true | None -> { let x = false; } }",
    ] {
        assert!(compile(&model(&format!("actor API {{ init(): Bool {{ false }} handle_message(state: Bool, msg: Option<Bool>): Bool {{ {body} }} }}")), None).is_err());
    }
}
#[test]
fn actor_reference_type_is_not_a_state_capability_or_compatibility_alias() {
    let source = include_str!("../examples/routed-deposits.fml");
    assert!(compile(source, None).is_ok());
    for ty in ["Address<Account>", "Actor<Int>", "Actor<Missing>", "Actor"] {
        assert!(
            compile(&source.replace("Actor<Account>", ty), None).is_err(),
            "{ty}"
        );
    }
}

#[test]
fn property_helpers_support_local_let_but_properties_are_expressions() {
    let source = r#"
actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: unit): Int { state + 1 }
}
let small = (candidate: Option<Int>): Bool {
  match candidate { | None -> true | Some(value) -> value <= 1 }
}
let bounded = (): Bool {
  let current = instances(Counter);
  let valid = forall (counter in current) { small(counter.state) };
  valid
}
property "safe" { always bounded() }
check C { spawn_bound Counter = 1 mailbox_bound = 1 domain Int = 0..1 main { let counter = spawn(Counter); inputs { once send(counter, ()) } } }
"#;
    let p = compile(source, None).unwrap();
    assert_eq!(
        checker::check(source, &p, &Options::default())
            .unwrap()
            .status,
        Status::VerifiedInScope
    );
    assert!(
        compile(
            &source.replace("always bounded()", "let small = bounded(); always small"),
            None
        )
        .is_err()
    );
    assert!(compile(&format!("let limit = 1\n{source}"), None).is_err());
    assert!(
        compile(
            &source.replace("{ state + 1 }", "{ let safe = bounded(); state }"),
            None
        )
        .is_err()
    );
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
    assert!(compile("actor A { handle_message(msg: Bool): unit { () } } property \"p\" { always (forall (A in Bool) { A }) } check C { spawn_bound A = 0 mailbox_bound = 1 main {} }", None).is_err());
}

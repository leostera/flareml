use flareml::{
    checker::{self, Options, Status},
    compile,
};
const STATELESS: &str = include_str!("../examples/actor-stateless.fml");
const STATEFUL: &str = include_str!("../examples/actor-counter.fml");
const CALL: &str = include_str!("../examples/actor-call.fml");
const KEYED: &str = include_str!("../examples/actor-keyed.fml");
const INTERLEAVING: &str = include_str!("../examples/actor-interleaving.fml");
const ADDRESS: &str = include_str!("../examples/actor-address.fml");
fn run(source: &str) -> checker::Report {
    let p = compile(source, None).unwrap();
    checker::check(source, &p, &Options::default()).unwrap()
}
#[test]
fn pure_functions_compose_with_stateless_actors_and_temporal_claims() {
    let report = run(STATELESS);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(report.states, 9);
}
#[test]
fn stateful_actor_counterexample_replays() {
    let p = compile(STATEFUL, None).unwrap();
    let report = run(STATEFUL);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert_eq!(
        witness.states.last().unwrap().actors["Counter"],
        flareml::semantics::Value::Int(2)
    );
    witness.validate(STATEFUL, &p).unwrap();
}
#[test]
fn one_input_preserves_state_safety() {
    let source = STATEFUL.replace(
        "    once Counter.add(1)\n    once Counter.add(1)",
        "    once Counter.add(1)",
    );
    assert_eq!(run(&source).status, Status::VerifiedInScope);
}
#[test]
fn stateful_capability_must_match_and_cannot_be_returned() {
    let bad = STATEFUL.replace("owner: Actor<Int>", "owner: Actor<Reply>");
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("does not match")
    );
    let bad = STATEFUL.replace(
        "let increment = (owner: Actor<Int>, amount: Int): Reply",
        "let increment = (owner: Actor<Int>, amount: Int): Actor<Int>",
    );
    assert!(compile(&bad, None).is_err());
}
#[test]
fn owner_capabilities_cannot_escape_through_nested_data_types() {
    for declaration in [
        "type Leak = Leak(Actor<Int>)",
        "type Leak = Leak { owner: Option<Actor<Int>> }",
        "type Envelope = Envelope { owner: Actor<Int> }\ntype Leak = Leak(Envelope)",
    ] {
        let source = STATEFUL.replace(
            "type Reply = Count(Int)",
            &format!("type Reply = Count(Int)\n{declaration}"),
        );
        assert!(
            compile(&source, None)
                .unwrap_err()
                .message
                .contains("owned capability")
        );
    }
}
#[test]
fn recursive_pure_calls_are_rejected() {
    let source = STATELESS.replace("decision(request.eligible)", "login(request)");
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("recursive")
    );
}
#[test]
fn inspector_function_cannot_be_bound_as_handler() {
    let source = STATELESS.replace(
        "let login = (request: Request): Reply {\n  decision(request.eligible)",
        "let login = (request: Request): Reply {\n  requests(API.handle_request)",
    );
    assert!(compile(&source, None).is_err());
}
#[test]
fn actor_call_requires_v1_profile() {
    let source = STATELESS.replace(
        "decision(request.eligible)",
        "call(API.handle_request, request)",
    );
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("actors-v1")
    );
}
#[test]
fn actor_calls_suspend_resume_and_replay() {
    let p = compile(CALL, None).unwrap();
    let report = run(CALL);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert!(
        witness
            .actions
            .iter()
            .any(|a| a.description.contains("calls Counter.add"))
    );
    assert!(
        witness
            .actions
            .iter()
            .any(|a| a.description.contains("accept Counter.add"))
    );
    witness.validate(CALL, &p).unwrap();
    assert_eq!(witness.format_version, 3);
    let mut previous = witness.clone();
    previous.format_version = 2;
    assert!(
        previous
            .validate(CALL, &p)
            .unwrap_err()
            .message
            .contains("unsupported format")
    );
}
#[test]
fn actor_call_returns_under_weak_fairness() {
    let source = CALL.replace("once API.add(1) once API.add(1)", "once API.add(1)");
    assert_eq!(run(&source).status, Status::VerifiedInScope);
}
#[test]
fn actor_call_is_typed_and_profile_gated() {
    let source = CALL.replace("call(Counter.add, amount)", "call(Counter.add, Count(1))");
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("type mismatch")
    );
    let source = CALL.replace("actors-v1", "actors-v0");
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("actors-v1")
    );
}
#[test]
fn keyed_instances_are_isolated_and_replayable() {
    let p = compile(KEYED, None).unwrap();
    let report = run(KEYED);
    assert_eq!(report.status, Status::VerifiedInScope);
    let initial = p.initial().unwrap();
    assert_eq!(initial.keyed_actors["Account"].len(), 2);
    assert!(
        initial.keyed_actors["Account"]
            .values()
            .all(|v| *v == flareml::semantics::Value::Int(0))
    );
    let source = KEYED.replace(
        "once API.deposit(Alice) once API.deposit(Bob)",
        "once API.deposit(Alice) once API.deposit(Alice)",
    );
    let p = compile(&source, None).unwrap();
    let report = run(&source);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    assert!(
        witness
            .actions
            .iter()
            .any(|a| a.description.contains("Account.at(Alice).deposit"))
    );
    witness.validate(&source, &p).unwrap();
    let encoded = serde_json::to_string(witness).unwrap();
    let decoded: flareml::trace::Trace = serde_json::from_str(&encoded).unwrap();
    decoded.validate(&source, &p).unwrap();
    let last = witness.states.last().unwrap();
    use flareml::semantics::Value;
    assert_eq!(
        last.keyed_actors["Account"][&Value::Variant("Alice".into(), vec![])],
        Value::Int(2)
    );
    assert_eq!(
        last.keyed_actors["Account"][&Value::Variant("Bob".into(), vec![])],
        Value::Int(0)
    );
}
#[test]
fn keyed_identity_is_typed_and_profile_gated() {
    assert!(
        compile(&KEYED.replace("Account.at(id)", "Account.at(1)"), None)
            .unwrap_err()
            .message
            .contains("type mismatch")
    );
    assert!(
        compile(&KEYED.replace("actors-v1", "actors-v0"), None)
            .unwrap_err()
            .message
            .contains("actors-v1")
    );
    assert!(
        compile(
            &KEYED.replace(
                "call(Account.at(id).deposit, 1)",
                "call(Account.deposit, 1)"
            ),
            None
        )
        .unwrap_err()
        .message
        .contains("keyed call")
    );
}
#[test]
fn keyed_direct_input_is_a_distinct_identity() {
    let source = KEYED.replace(
        "once API.deposit(Alice) once API.deposit(Bob)",
        "once Account.at(Alice).deposit(1)",
    );
    let p = compile(&source, None).unwrap();
    let report = run(&source);
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(
        p.initial().unwrap().frames[0].key,
        Some(flareml::semantics::Value::Variant("Alice".into(), vec![]))
    );
    let bad = source.replace("Account.at(Alice).deposit(1)", "Account.deposit(1)");
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("keyed actor input")
    );
}
#[test]
fn cyclic_actor_calls_are_bounded_not_mistaken_for_local_recursion() {
    let source = CALL
        .replace("call(Counter.add, amount)", "call(API.add, amount)")
        .replace("once API.add(1) once API.add(1)", "once API.add(1)");
    let p = compile(&source, None).unwrap();
    let report = checker::check(&source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert!(
        report
            .cutoff
            .as_deref()
            .unwrap_or_default()
            .contains("actor call")
    );
}
#[test]
fn same_key_interleaves_while_calling_another_actor() {
    let p = compile(INTERLEAVING, None).unwrap();
    let report = run(INTERLEAVING);
    assert_eq!(report.status, Status::Violated);
    let witness = report.witness().unwrap();
    witness.validate(INTERLEAVING, &p).unwrap();
    assert_eq!(
        witness.states.last().unwrap().keyed_actors["Counter"]
            [&flareml::semantics::Value::Variant("Shared".into(), vec![])],
        flareml::semantics::Value::Int(1)
    );
}
#[test]
fn typed_addresses_can_cross_messages_and_route_calls() {
    let p = compile(ADDRESS, None).unwrap();
    let report = run(ADDRESS);
    assert_eq!(report.status, Status::VerifiedInScope);
    let input = &p.initial().unwrap().frames[0].input;
    assert!(matches!(input, flareml::semantics::Value::Record(_, fields)
        if matches!(fields.get("target"), Some(flareml::semantics::Value::Address(_, _)))));
    let bad = ADDRESS.replace("Account.at(Bob), amount: 1", "Account.at(Alice), amount: 1");
    let p = compile(&bad, None).unwrap();
    let report = run(&bad);
    assert_eq!(report.status, Status::Violated);
    report.witness().unwrap().validate(&bad, &p).unwrap();
}
#[test]
fn addresses_are_not_owner_capabilities() {
    let bad = ADDRESS.replace("target: Address<Account>", "target: Actor<Int>");
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("owned capability")
    );
    let bad = ADDRESS.replace("target: Account.at(Alice)", "target: Account.at(1)");
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("type mismatch")
    );
    let bad = ADDRESS.replace("input.target.deposit", "input.target.unknown");
    assert!(
        compile(&bad, None)
            .unwrap_err()
            .message
            .contains("known actor handler")
    );
}
#[test]
fn actor_models_do_not_claim_legacy_profile() {
    let source = STATELESS.replace("actors-v0", "cf-core-v0");
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("require")
    );
}

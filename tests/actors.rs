use flareml::{
    checker::{self, Options, Status},
    compile,
};
const STATELESS: &str = include_str!("../examples/actor-stateless.fml");
const STATEFUL: &str = include_str!("../examples/actor-counter.fml");
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
fn cross_actor_call_is_rejected_until_semantics_exist() {
    let source = STATELESS.replace(
        "decision(request.eligible)",
        "call(API.handle_request, request)",
    );
    assert!(compile(&source, None).is_err()); // recursive or unsupported; never silently checked
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

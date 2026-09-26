use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Value,
};
const ROUTED: &str = include_str!("../examples/routed-deposits.fml");
const POLICY: &str = include_str!("../examples/eligibility-check.fml");
fn run(source: &str) -> checker::Report {
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Options::default()).unwrap();
    for c in &r.claims {
        if let Some(t) = &c.witness {
            t.validate(source, &p).unwrap();
        }
    }
    r
}
#[test]
fn pure_functions_compose_with_actors() {
    assert_eq!(run(POLICY).status, Status::VerifiedInScope);
}
#[test]
fn policy_bug_has_a_short_witness() {
    let source = POLICY.replace("| Ineligible -> Denied", "| Ineligible -> Allowed");
    let r = run(&source);
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.witness().unwrap().actions.len(), 2);
}
#[test]
fn explicit_instances_and_typed_routes_are_isolated() {
    let p = compile(ROUTED, None).unwrap();
    let initial = p.initial().unwrap();
    assert_eq!(initial.spawned["Account"].len(), 2);
    assert!(
        initial.spawned["Account"]
            .iter()
            .all(|v| *v == Value::Int(0))
    );
    assert_eq!(run(ROUTED).status, Status::VerifiedInScope);
}
#[test]
fn misrouting_is_not_hidden_by_equal_message_payloads() {
    let source = ROUTED.replace("target: bob, amount: 1", "target: alice, amount: 1");
    let r = run(&source);
    assert_eq!(r.status, Status::Violated);
    let final_state = r.witness().unwrap().states.last().unwrap();
    assert_eq!(final_state.spawned["Account"][0], Value::Int(2));
    assert_eq!(final_state.spawned["Account"][1], Value::Int(0));
}
#[test]
fn address_types_cannot_be_forged_or_used_as_state_capabilities() {
    for source in [
        ROUTED.replace("target: alice", "target: Account.at(1)"),
        ROUTED.replace("target: Actor<Account>", "target: Actor<Router>"),
        ROUTED.replace(
            "send(message.target, Deposit(message.amount))",
            "message.target.set(1)",
        ),
        ROUTED.replace(
            "send(message.target, Deposit(message.amount))",
            "message.target.state",
        ),
        ROUTED.replace("amount: 1", "amount: true"),
    ] {
        assert!(compile(&source, None).is_err(), "{source}");
    }
}
#[test]
fn recursive_local_helpers_are_rejected_but_message_cycles_are_not_recursion() {
    let source = POLICY.replace("| Eligible -> Allowed", "| Eligible -> decide(request)");
    assert!(
        compile(&source, None)
            .unwrap_err()
            .message
            .contains("recursive")
    );
    let cycle = "actor A { handle_message(me: Actor<A>): unit { send(me, me) } } property \"progress\" { forall (i in inputs(A)) { i.submitted leads_to i.processed } } check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); inputs { once send(a, a) } } fairness { weak runtime.progress } }";
    assert_eq!(run(cycle).status, Status::VerifiedInScope);
}
#[test]
fn payload_aliases_are_transparent() {
    let source = ROUTED
        .replace("type Route =", "type Amount = Int\ntype Route =")
        .replace("amount: Int", "amount: Amount");
    assert_eq!(run(&source).status, Status::VerifiedInScope);
}
#[test]
fn old_language_constructs_are_not_compatibility_modes() {
    for source in [
        "worker A { go(x: Bool): Bool { respond(x) } }",
        "stateless actor A { go = f }",
        "stateful actor A { state: Bool = false go = f }",
        "d1 DB { table Item { id: Bool primary_key } }",
    ] {
        assert!(
            compile(
                &format!(
                    "{source} property \"p\" {{ always true }} check C {{ mailbox_bound = 1 }}"
                ),
                None
            )
            .is_err()
        );
    }
    for call in [
        "call(Policy.handle_message, request)",
        "respond(Denied)",
        "requests(Policy.handle_message)",
    ] {
        assert!(
            compile(
                &POLICY.replace("decide(request) }", &format!("{call} }}")),
                None
            )
            .is_err()
        );
    }
}
#[test]
fn lost_update_and_atomic_repair_have_different_verdicts() {
    let r = run(include_str!("../examples/lost-update.fml"));
    assert_eq!(r.status, Status::Violated);
    assert_eq!(
        r.witness().unwrap().states.last().unwrap().spawned["Store"][0],
        Value::Int(1)
    );
    assert_eq!(
        run(include_str!("../examples/atomic-increments.fml")).status,
        Status::VerifiedInScope
    );
}
#[test]
fn lost_updates_can_violate_safety_even_when_each_client_finishes() {
    let source = include_str!("../examples/lost-update.fml");
    let p = compile(source, None).unwrap();
    let r = checker::check(
        source,
        &p,
        &Options {
            property: Some("submitted clients finish".into()),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(r.status, Status::VerifiedInScope);
}

#[test]
fn singleton_sequential_workflow_is_a_finite_model() {
    let source = include_str!("../examples/sequential-workflow.fml");
    let r = run(source);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert!(r.states < 10);
    assert_eq!(
        run(&source.replace("fairness { weak runtime.progress }", "")).status,
        Status::Violated
    );
}

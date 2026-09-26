//! Public source-to-checker contract for the unified property surface.
use flareml::{
    checker::{self, Options, Status},
    compile,
    syntax::ClaimKind,
};
const BASE: &str = r#"
actor Machine {
  init(): Bool { false }
  handle_message(state: Bool, message: Actor<Machine>): Bool { true }
}
let machine_value = (): Bool { forall (machine in instances(Machine)) { machine.state == Some(true) } }
CLAIM
check C {
  spawn_bound Machine = 1
  mailbox_bound = 1
  main { let machine = spawn(Machine); inputs { once send(machine, machine) } }
  fairness { weak runtime.progress }
}
"#;
fn check(claim: &str, max_states: usize) -> checker::Report {
    let source = BASE.replace("CLAIM", claim);
    let p = compile(&source, None).unwrap();
    let report = checker::check(
        &source,
        &p,
        &Options {
            max_states,
            ..Options::default()
        },
    )
    .unwrap();
    for result in &report.claims {
        if let Some(trace) = &result.witness {
            trace.validate(&source, &p).unwrap();
            let decoded: flareml::trace::Trace =
                serde_json::from_str(&serde_json::to_string(trace).unwrap()).unwrap();
            decoded.validate(&source, &p).unwrap();
        }
    }
    report
}
#[test]
fn safety_checks_initial_state_before_exploration_cutoff() {
    let r = check("property \"safe\" { always machine_value() }", 1);
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.claims[0].kind, ClaimKind::Invariant);
    assert!(r.witness().unwrap().actions.is_empty());
}
#[test]
fn discovered_bad_state_is_not_hidden_by_a_sibling_search_cutoff() {
    let source = BASE
        .replace(
            "CLAIM",
            "property \"safe\" { always (forall (i in inputs(Machine)) { !i.submitted }) }",
        )
        .replace(
            "once send(machine, machine)",
            "once send(machine, machine) once send(machine, machine)",
        );
    let p = compile(&source, None).unwrap();
    let r = checker::check(
        &source,
        &p,
        &Options {
            max_states: 2,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.witness().unwrap().actions.len(), 1);
    r.witness().unwrap().validate(&source, &p).unwrap();
}
#[test]
fn safety_checks_reachable_states_before_later_capacity_cutoff() {
    let source = BASE
        .replace("CLAIM", "property \"safe\" { always (!machine_value()) }")
        .replace("{ true }", "{ send(message, message); true }")
        .replace("mailbox_bound = 1", "mailbox_bound = 1 message_bound = 2");
    let p = compile(&source, None).unwrap();
    let r = checker::check(&source, &p, &Options::default()).unwrap();
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.witness().unwrap().actions.len(), 2);
    r.witness().unwrap().validate(&source, &p).unwrap();
}
#[test]
fn reachable_is_not_eventually_and_unused_inputs_are_optional() {
    let r = check(
        "property \"possible\" { reachable machine_value() }\nproperty \"inevitable\" { eventually machine_value() }",
        100,
    );
    assert_eq!(r.claims[0].kind, ClaimKind::Cover);
    assert_eq!(r.claims[0].result, "REACHED");
    assert_eq!(r.claims[1].result, "VIOLATED");
    assert!(r.claims[1].witness.as_ref().unwrap().loop_start.is_some());
}
#[test]
fn zero_step_reachability_survives_incomplete_exploration() {
    let r = check("property \"possible\" { reachable (!machine_value()) }", 1);
    assert_eq!(r.status, Status::Inconclusive);
    assert_eq!(r.claims[0].result, "REACHED");
    assert!(r.witness().unwrap().actions.is_empty());
    assert!(!r.complete);
}
#[test]
fn absence_is_unreachable_only_on_a_closed_graph_and_is_not_failure() {
    let claim = "property \"impossible\" { reachable false }";
    assert_eq!(check(claim, 1).claims[0].result, "INCONCLUSIVE");
    let complete = check(claim, 100);
    assert_eq!(complete.status, Status::VerifiedInScope);
    assert_eq!(complete.claims[0].result, "UNREACHABLE");
}
#[test]
fn temporal_nesting_is_not_misclassified_as_safety() {
    for formula in [
        "always eventually machine_value()",
        "eventually always machine_value()",
        "forall (i in inputs(Machine)) { i.submitted leads_to i.processed }",
    ] {
        assert_eq!(
            check(&format!("property \"temporal\" {{ {formula} }}"), 100).claims[0].kind,
            ClaimKind::Property
        );
    }
}
#[test]
fn reachability_accepts_data_quantifiers_but_rejects_path_mixtures() {
    assert_eq!(
        check(
            "property \"possible\" { reachable (exists (i in inputs(Machine)) { i.processed }) }",
            100
        )
        .claims[0]
            .result,
        "REACHED"
    );
    for formula in [
        "machine_value()",
        "reachable (eventually machine_value())",
        "always (reachable machine_value())",
        "reachable (reachable machine_value())",
        "(reachable machine_value()) && (eventually machine_value())",
        "forall (i in inputs(Machine)) { reachable i.processed }",
        "reachable 1",
        "reachable send(Machine, ())",
    ] {
        assert!(
            compile(
                &BASE.replace("CLAIM", &format!("property \"bad\" {{ {formula} }}")),
                None
            )
            .is_err(),
            "accepted {formula}"
        );
    }
}
#[test]
fn obsolete_claim_declarations_are_rejected() {
    for claim in ["invariant \"safe\" { true }", "cover \"possible\" { true }"] {
        assert!(compile(&BASE.replace("CLAIM", claim), None).is_err());
    }
}
#[test]
fn semantics_are_not_user_selectable() {
    for version in ["cf-core-v0", "actors-v0", "actors-v1", "actors-v2"] {
        let source = format!(
            "property \"p\" {{ always true }} check C {{ semantics = \"{version}\" mailbox_bound = 1 main {{}} }}"
        );
        assert!(compile(&source, None).is_err());
    }
}
#[test]
fn selected_property_and_trace_versions_are_enforced() {
    let source = BASE.replace(
        "CLAIM",
        "property \"possible\" { reachable machine_value() }\nproperty \"safe\" { always false }",
    );
    let p = compile(&source, None).unwrap();
    let r = checker::check(
        &source,
        &p,
        &Options {
            property: Some("possible".into()),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.not_checked, ["safe"]);
    let mut trace = r.witness().unwrap().clone();
    assert_eq!(trace.format_version, flareml::trace::FORMAT_VERSION);
    trace.format_version = 5;
    assert!(trace.validate(&source, &p).is_err());
    trace.format_version = flareml::trace::FORMAT_VERSION;
    trace.kind = ClaimKind::Property;
    assert!(trace.validate(&source, &p).is_err());
}

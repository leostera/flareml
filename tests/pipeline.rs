use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::{Env, Value},
    syntax::{ClaimKind, Expr, ExprKind, Span},
    trace::Trace,
};
use std::time::Duration;
const BUG: &str = include_str!("../examples/login-bug.fml");
const FIXED: &str = include_str!("../examples/login-fixed.fml");
const STARVATION: &str = include_str!("../examples/starvation.fml");
fn run(source: &str) -> checker::Report {
    let p = compile(source, None).unwrap();
    checker::check(source, &p, &Options::default()).unwrap()
}
fn tiny(claim: &str) -> String {
    format!("{claim}\ncheck Main {{ semantics = \"cf-core-v0\" }}")
}

#[test]
fn login_bug_has_short_source_mapped_witness() {
    let r = run(BUG);
    assert_eq!(r.status, Status::Violated);
    let w = r.witness().unwrap();
    assert_eq!(w.actions.len(), 4);
    assert!(w.actions.last().unwrap().description.contains("Allowed"));
    assert!(w.actions.last().unwrap().span.start > 0);
    assert_eq!(w.kind, ClaimKind::Invariant);
}
#[test]
fn login_fixed_checks_all_interleavings() {
    let r = run(FIXED);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert!(r.complete);
    assert_eq!(r.states, 25);
    assert!(r.claims.iter().any(|c| c.result == "REACHED"));
}
#[test]
fn lack_of_fairness_has_a_valid_lasso() {
    let r = run(STARVATION);
    assert_eq!(r.status, Status::Violated);
    let t = r.witness().unwrap();
    assert!(t.loop_start.is_some());
    t.validate(STARVATION, &compile(STARVATION, None).unwrap())
        .unwrap();
}
#[test]
fn traces_round_trip_but_reject_tampering() {
    for src in [BUG, STARVATION] {
        let p = compile(src, None).unwrap();
        let t = run(src).witness().unwrap().clone();
        let json = serde_json::to_string(&t).unwrap();
        let decoded: Trace = serde_json::from_str(&json).unwrap();
        decoded.validate(src, &p).unwrap();
        let mut broken = t.clone();
        broken.actions[0].id = "fake".into();
        assert!(broken.validate(src, &p).is_err());
        let mut broken = t.clone();
        broken.states[0].tables.clear();
        assert!(broken.validate(src, &p).is_err());
        let mut broken = t.clone();
        broken.format_version = 999;
        assert!(broken.validate(src, &p).is_err());
        assert!(t.validate(&format!("{src}\n"), &p).is_err());
    }
}
#[test]
fn replay_rejects_invalid_loop() {
    let mut t = run(STARVATION).witness().unwrap().clone();
    t.loop_start = Some(usize::MAX);
    assert!(
        t.validate(STARVATION, &compile(STARVATION, None).unwrap())
            .is_err()
    );
}
#[test]
fn cutoffs_are_inconclusive() {
    let p = compile(FIXED, None).unwrap();
    for options in [
        Options {
            max_states: 1,
            ..Options::default()
        },
        Options {
            max_depth: 1,
            ..Options::default()
        },
        Options {
            timeout: Duration::ZERO,
            ..Options::default()
        },
    ] {
        let r = checker::check(FIXED, &p, &options).unwrap();
        assert_eq!(r.status, Status::Inconclusive);
        assert!(!r.complete);
        assert!(r.cutoff.is_some());
    }
}
#[test]
fn initial_invariant_is_checked() {
    let src = tiny("invariant \"initial\" { false }");
    let r = run(&src);
    assert_eq!(r.status, Status::Violated);
    assert!(r.witness().unwrap().actions.is_empty());
}
#[test]
fn terminal_eventuality_needs_a_lasso() {
    let src = tiny("property \"never\" { eventually false }");
    let r = run(&src);
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.witness().unwrap().loop_start, Some(0));
}
#[test]
fn temporal_operators_boundary_cases() {
    for (formula, pass) in [
        ("always true", true),
        ("always false", false),
        ("eventually true", true),
        ("eventually false", false),
        ("true leads_to true", true),
        ("true leads_to false", false),
        ("false leads_to false", true),
        ("false until true", true),
        ("true until false", false),
        ("false until false", false),
        ("always eventually true", true),
        ("always eventually false", false),
        ("eventually always true", true),
        ("eventually always false", false),
        ("always (true implies always true)", true),
        ("always (true implies always false)", false),
        ("always (false implies always false)", true),
        ("(always true) && (eventually true)", true),
        ("(always true) && (eventually false)", false),
    ] {
        let src = tiny(&format!("property \"temporal\" {{ {formula} }}"));
        let r = run(&src);
        assert_eq!(r.status == Status::VerifiedInScope, pass, "{formula}");
        if let Some(t) = r.witness() {
            t.validate(&src, &compile(&src, None).unwrap()).unwrap();
        }
    }
}
#[test]
fn temporal_operators_on_changing_states() {
    for formula in [
        "r.accepted leads_to r.completed",
        "eventually always (r.completed || not r.accepted)",
        "always eventually (r.completed || not r.accepted)",
        "always (r.completed implies always r.completed)",
    ] {
        let src = FIXED.replace("r.accepted leads_to r.completed", formula);
        assert_eq!(run(&src).status, Status::VerifiedInScope, "{formula}");
    }
}
#[test]
fn property_selection_is_explicit() {
    let p = compile(BUG, None).unwrap();
    let r = checker::check(
        BUG,
        &p,
        &Options {
            property: Some("an accepted login eventually responds".into()),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(r.status, Status::VerifiedInScope);
    assert_eq!(r.claims.len(), 1);
    assert!(
        checker::check(
            BUG,
            &p,
            &Options {
                property: Some("missing".into()),
                ..Options::default()
            }
        )
        .is_err()
    );
}
#[test]
fn unsupported_temporal_forms_are_rejected() {
    for formula in [
        "always eventually always true",
        "(always true) || (eventually false)",
        "not (always false)",
        "exists (r in requests(LoginAPI.handle_request)) { eventually r.completed }",
    ] {
        let src=FIXED.replace("forall (r in requests(LoginAPI.handle_request)) {\n    r.accepted leads_to r.completed\n  }",formula);
        assert!(compile(&src, None).is_err(), "{formula}");
    }
}
#[test]
fn effects_cannot_hide_in_invariants_or_expressions() {
    for body in [
        "AppDB.User.get(Alice) == None",
        "respond(Allowed) == respond(Allowed)",
        "true || unknown_name",
    ] {
        let src = FIXED.replace(
            "cover \"an existing user can log in\"",
            &format!("invariant \"bad\" {{ {body} }}\ncover \"an existing user can log in\""),
        );
        assert!(compile(&src, None).is_err(), "{body}");
    }
}
#[test]
fn syntax_and_type_errors_fail_closed() {
    for src in [
        FIXED.replace("request.user_id", "request.missing"),
        FIXED.replace("| None -> respond(Denied)", ""),
        FIXED.replace("respond(Denied)", "respond(Alice)"),
        FIXED.replace("primary_key", ""),
        FIXED.replace("cf-core-v0", "invented-semantics"),
        FIXED.replace("weak runtime.progress", "strong runtime.progress"),
        FIXED.replace("weak runtime.progress", "weak everything"),
        format!("{FIXED}\nkv Cache : KV<UserId, UserId>"),
        format!("{FIXED}\nqueue Events : Queue<UserId> {{}}"),
    ] {
        assert!(compile(&src, None).is_err(), "{src}");
    }
}
#[test]
fn invalid_initial_schema_is_not_a_violation() {
    let src = FIXED.replace(
        "[User { id: Alice }]",
        "[User { id: Alice }, User { id: Alice }]",
    );
    let p = compile(&src, None).unwrap();
    assert!(
        checker::check(&src, &p, &Options::default())
            .unwrap_err()
            .message
            .contains("initial constraint")
    );
}
#[test]
fn empty_quantifier_is_reported_as_vacuous() {
    let src = FIXED
        .replace(
            "once LoginAPI.handle_request(LoginRequest { user_id: Alice })",
            "",
        )
        .replace(
            "once LoginAPI.handle_request(LoginRequest { user_id: Bob })",
            "",
        );
    let r = run(&src);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert!(
        r.claims
            .iter()
            .any(|c| c.note.as_ref().is_some_and(|n| n.contains("vacuous")))
    );
    assert!(r.claims.iter().any(|c| c.result == "UNREACHABLE"));
}
#[test]
fn multiple_checks_require_selection() {
    let src = format!("{FIXED}\ncheck Other {{ semantics = \"cf-core-v0\" }}");
    assert!(compile(&src, None).is_err());
    assert!(compile(&src, Some("Login")).is_ok());
    assert!(compile(&src, Some("missing")).is_err());
}
#[test]
fn aliases_and_string_domains() {
    let src = FIXED
        .replace("type UserId = Alice | Bob", "type UserId = String")
        .replace("Alice", "\"alice\"")
        .replace("Bob", "\"bob\"")
        .replace(
            "check Login {",
            "check Login { domain String = [\"alice\", \"bob\"]",
        );
    assert_eq!(run(&src).status, Status::VerifiedInScope);
}
#[test]
fn deeply_nested_input_is_rejected() {
    let src = tiny(&format!(
        "invariant \"deep\" {{ {}true{} }}",
        "(".repeat(150),
        ")".repeat(150)
    ));
    assert!(compile(&src, None).unwrap_err().message.contains("nesting"));
}
#[test]
fn unicode_strings_preserve_byte_spans() {
    let src = tiny("invariant \"不存在\" { false }");
    let r = run(&src);
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.claims[0].name, "不存在");
}

const MUTATION: &str = r#"
type Id = A
type Reply = Done | Rejected
d1 DB { table Counter { id: Id primary_key value: Int } }
worker API {
  change(request: Id): Reply {
    let first = DB.Counter.update(request, Counter { id: request, value: 1 });
    match first {
      | Err(_) -> respond(Rejected)
      | Ok(_) -> {
        let second = DB.Counter.update(request, Counter { id: request, value: 2 });
        match second { | Err(_) -> respond(Rejected) | Ok(_) -> respond(Done) }
      }
    }
  }
}
invariant "intermediate writes are observable" {
  forall (row in DB.Counter.rows) { row.value == 0 || row.value == 2 }
}
check Main {
 semantics = "cf-core-v0"
 domain Int = 0..2
 init { DB.Counter = [Counter { id: A, value: 0 }] }
 inputs { once API.change(A) }
 fairness { weak runtime.progress }
}
"#;
#[test]
fn invariant_checked_between_writes_in_one_handler() {
    let r = run(MUTATION);
    assert_eq!(r.status, Status::Violated);
    let t = r.witness().unwrap();
    assert!(
        t.actions
            .last()
            .unwrap()
            .description
            .contains("update completes")
    );
    assert!(t.actions.len() < 7);
}
#[test]
fn scope_escape_is_inconclusive_not_wrapped_or_pruned() {
    let src = MUTATION.replace("value: 1", "value: 3");
    let r = run(&src);
    assert_eq!(r.status, Status::Inconclusive);
    assert!(r.cutoff.unwrap().contains("escapes domain"));
}
#[test]
fn pure_arithmetic_uses_checked_integers() {
    let p = compile(FIXED, None).unwrap();
    let s = p.initial().unwrap();
    let span = Span::default();
    let e = Expr {
        kind: ExprKind::Binary(
            "+".into(),
            Box::new(Expr {
                kind: ExprKind::Int(i64::MAX),
                span,
            }),
            Box::new(Expr {
                kind: ExprKind::Int(1),
                span,
            }),
        ),
        span,
    };
    assert!(
        p.eval(&e, &Env::new(), &s)
            .unwrap_err()
            .message
            .contains("overflow")
    );
}
#[test]
fn property_views_do_not_execute_reads() {
    let p = compile(FIXED, None).unwrap();
    let s = p.initial().unwrap();
    let before = s.clone();
    let _ = p
        .predicate(&p.model.claims[0].body, &Env::new(), &s)
        .unwrap();
    assert_eq!(s, before);
    assert_eq!(s.frames[0].response, Value::none());
}

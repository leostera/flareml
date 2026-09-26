use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Env,
    syntax::{Expr, ExprKind, Span},
    trace::Trace,
};
use std::time::Duration;
const FIXED: &str = include_str!("../examples/counter-replies.fml");
const BUG: &str = include_str!("../examples/missing-reply.fml");
fn run(source: &str) -> checker::Report {
    checker::check(source, &compile(source, None).unwrap(), &Options::default()).unwrap()
}
fn tiny(claim: &str) -> String {
    format!("{claim}\ncheck Main {{ mailbox_bound = 1 main {{}} }}")
}

#[test]
fn missing_reply_has_a_source_mapped_fair_lasso() {
    let r = run(BUG);
    assert_eq!(r.status, Status::Violated);
    let w = r.witness().unwrap();
    assert_eq!(w.actions.len(), 4);
    assert_eq!(w.loop_start, Some(3));
    assert!(w.actions[1].span.start > 0);
    w.validate(BUG, &compile(BUG, None).unwrap()).unwrap();
}
#[test]
fn reply_protocol_explores_submission_commit_and_delivery() {
    let r = run(FIXED);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert!(r.complete);
    assert_eq!((r.states, r.edges), (4, 7));
    assert!(r.claims.iter().any(|c| c.result == "REACHED"));
}
#[test]
fn traces_round_trip_but_reject_tampering() {
    for src in [BUG, FIXED] {
        let p = compile(src, None).unwrap();
        let t = run(src).witness().unwrap().clone();
        let decoded: Trace = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        decoded.validate(src, &p).unwrap();
        for field in 0..8 {
            let mut broken = t.clone();
            match field {
                0 => broken.actions[0].id = "fake".into(),
                1 => broken.states[0].mailboxes.clear(),
                2 => broken.format_version = 999,
                3 => broken.loop_start = Some(usize::MAX),
                4 => broken.actions[0].fair = true,
                5 => broken.actions[0].span.start += 1,
                6 => broken.actions[0].description.push('!'),
                _ => broken.claim = "absent".into(),
            }
            assert!(broken.validate(src, &p).is_err(), "mutation {field}");
        }
        assert!(t.validate(&format!("{src}\n"), &p).is_err());
    }
}
#[test]
fn exploration_cutoffs_are_inconclusive() {
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
        "m.sent leads_to m.processed",
        "eventually always (m.processed || not m.sent)",
        "always eventually (m.processed || not m.sent)",
        "always (m.processed implies always m.processed)",
    ] {
        assert_eq!(
            run(&FIXED.replace("m.sent leads_to m.processed", formula)).status,
            Status::VerifiedInScope,
            "{formula}"
        );
    }
}
#[test]
fn unsupported_temporal_forms_are_rejected() {
    for formula in [
        "always eventually always true",
        "(always true) || (eventually false)",
        "not (always false)",
        "exists (m in messages(Client)) { eventually m.processed }",
        "forall (x in [true, false]) { eventually x }",
    ] {
        let src = format!("{FIXED}\nproperty \"unsupported\" {{ {formula} }}");
        assert!(compile(&src, None).is_err(), "{formula}");
    }
}
#[test]
fn effects_and_unknown_names_cannot_hide_in_properties() {
    for body in [
        "forall (client in instances(Client)) { send(client.reference, Counted(First, 0)) == () }",
        "true || unknown_name",
    ] {
        assert!(
            compile(
                &format!("{FIXED}\nproperty \"bad\" {{ always ({body}) }}"),
                None
            )
            .is_err()
        );
    }
}
#[test]
fn syntax_and_type_errors_fail_closed() {
    for src in [
        FIXED.replace(
            "send(counter, Inc(client, First))",
            "send(client, Inc(client, First))",
        ),
        FIXED.replace("| Counted(_, _) -> Observed", ""),
        FIXED.replace("| Counted(_, _) -> Observed", "| Counted(_, _) -> First"),
        FIXED.replace("weak runtime.progress", "strong runtime.progress"),
        FIXED.replace("weak runtime.progress", "weak everything"),
        format!("{FIXED}\nkv Cache : KV<Bool, Bool>"),
        format!("{FIXED}\nqueue Events : Queue<Bool> {{}}"),
    ] {
        assert!(compile(&src, None).is_err(), "{src}");
    }
}
#[test]
fn empty_temporal_quantifier_reports_vacuity() {
    let src = FIXED.replace("once send(counter, Inc(client, First))", "");
    let r = run(&src);
    assert_eq!(r.status, Status::VerifiedInScope);
    assert!(r.claims.iter().any(|c| {
        c.note
            .as_ref()
            .is_some_and(|n| n.contains("empty temporal"))
    }));
    assert!(r.claims.iter().any(|c| c.result == "UNREACHABLE"));
}
#[test]
fn multiple_checks_and_properties_require_explicit_selection() {
    let src = format!(
        "{FIXED}\ncheck Other {{ spawn_bound Counter = 0 spawn_bound Client = 0 mailbox_bound = 1 main {{}} }}"
    );
    assert!(compile(&src, None).is_err());
    assert!(compile(&src, Some("OneIncrement")).is_ok());
    assert!(compile(&src, Some("missing")).is_err());
    let p = compile(FIXED, None).unwrap();
    assert!(
        checker::check(
            FIXED,
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
fn unicode_strings_and_source_spans_are_preserved() {
    let src = tiny("property \"不存在\" { always false }");
    let r = run(&src);
    assert_eq!(r.status, Status::Violated);
    assert_eq!(r.claims[0].name, "不存在");
    r.witness()
        .unwrap()
        .validate(&src, &compile(&src, None).unwrap())
        .unwrap();
}
#[test]
fn pure_arithmetic_uses_checked_integers() {
    let p = compile(FIXED, None).unwrap();
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
        p.eval(&e, &Env::new(), &p.initial().unwrap())
            .unwrap_err()
            .message
            .contains("overflow")
    );
}
#[test]
fn property_observations_cannot_mutate_state() {
    let p = compile(FIXED, None).unwrap();
    let s = p.initial().unwrap();
    let before = s.clone();
    p.predicate(&p.model.claims[0].body, &Env::new(), &s)
        .unwrap();
    assert_eq!(s, before);
    assert!(s.messages.is_empty());
    assert!(s.input_submitted.iter().all(|x| !x));
}

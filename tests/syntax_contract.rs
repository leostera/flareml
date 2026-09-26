//! Lock down the user-visible surface rather than only successful examples.
use flareml::{
    checker::{self, Options, Status},
    compile,
    syntax::{self, ExprKind},
};
fn model(property: &str) -> String {
    format!("property \"p\" {{ {property} }} check C {{ mailbox_bound = 1 }}")
}
#[test]
fn prefix_temporal_operators_require_parentheses_for_compound_predicates() {
    for operator in ["always", "reachable"] {
        let source = model(&format!("{operator} (1 + 2 == 3)"));
        let p = compile(&source, None).unwrap();
        assert_eq!(
            checker::check(&source, &p, &Options::default())
                .unwrap()
                .status,
            Status::VerifiedInScope
        );
        assert!(compile(&model(&format!("{operator} 1 + 2 == 3")), None).is_err());
    }
}
#[test]
fn arithmetic_comparison_boolean_and_temporal_precedence_is_explicit() {
    let source = model("1 + 2 < 4 && true || false implies true leads_to false");
    let ast = syntax::parse(&source).unwrap();
    let ExprKind::Binary(op, lhs, _) = &ast.claims[0].body.kind else {
        panic!("not binary")
    };
    assert_eq!(op, "leads_to");
    let ExprKind::Binary(op, lhs, _) = &lhs.kind else {
        panic!("not implication")
    };
    assert_eq!(op, "implies");
    let ExprKind::Binary(op, _, _) = &lhs.kind else {
        panic!("not disjunction")
    };
    assert_eq!(op, "||");
    let p = compile(&source, None).unwrap();
    assert_eq!(
        checker::check(&source, &p, &Options::default())
            .unwrap()
            .status,
        Status::Violated
    );
}
#[test]
fn keywords_cannot_be_declared_as_unusable_names() {
    for name in [
        "true",
        "false",
        "let",
        "actor",
        "property",
        "reachable",
        "always",
        "match",
        "forall",
    ] {
        let source = format!(
            "let {name} = (x: Bool): Bool {{ x }} {}",
            model("always true")
        );
        assert!(
            compile(&source, None)
                .unwrap_err()
                .message
                .contains("reserved keyword")
        );
    }
}
#[test]
fn duplicate_sections_and_out_of_range_bounds_are_rejected() {
    for config in [
        "",
        "mailbox_bound = 0",
        "mailbox_bound = 4097",
        "mailbox_bound = 1 mailbox_bound = 2",
        "mailbox_bound = 1 message_bound = 0",
        "mailbox_bound = 1 message_bound = 4097",
        "mailbox_bound = 1 domain Int = [0] domain Int = [1]",
        "mailbox_bound = 1 inputs {} inputs {}",
        "mailbox_bound = 1 init {}",
    ] {
        assert!(
            compile(
                &format!("property \"p\" {{ always true }} check C {{ {config} }}"),
                None
            )
            .is_err(),
            "{config}"
        );
    }
}
#[test]
fn actor_methods_and_arity_are_closed() {
    for actor in [
        "actor A {}",
        "actor A { handle_call(x: Bool): Bool { x } }",
        "actor A { handle_message(): unit { () } }",
        "actor A { init(x: Bool): Bool { x } handle_message(s: Bool, x: Bool): Bool { x } }",
        "actor A { handle_message(x: Bool): unit { () } handle_message(y: Bool): unit { () } }",
        "actor A { init(): Bool { false } init(): Bool { true } handle_message(s: Bool, x: Bool): Bool { x } }",
    ] {
        assert!(compile(&format!("{actor} {}", model("always true")), None).is_err());
    }
}
#[test]
fn singleton_type_and_constructor_share_a_name_without_losing_domain_identity() {
    let source = "type Tick = Tick property \"tick is present\" { reachable (exists (t in Tick) { t == Tick }) } property \"each tick\" { forall (t in Tick) { eventually (t == Tick) } } check C { mailbox_bound = 1 }";
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Options::default()).unwrap();
    assert_eq!(r.claims[0].result, "REACHED");
    assert!(r.claims[1].note.is_none());
    assert_eq!(r.status, Status::VerifiedInScope);
}

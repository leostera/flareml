use flareml::{
    checker::{self, Options, Status},
    compile,
    semantics::Value,
    syntax,
};

fn model(ty: &str, body: &str) -> String {
    format!(
        "actor A {{ init(): {ty} {{ {body} }} handle_message(s: {ty}, m: unit): {ty} {{ s }} }}
         property \"safe\" {{ always true }} check C {{ mailbox_bound = 1 }}"
    )
}

#[test]
fn bindings_and_non_tail_statements_require_separators() {
    for body in [
        "let x = true x",
        "let x = true\nx",
        "true false",
        "true\nfalse",
        "match Some(true) { | Some(x) -> x | None -> false } true",
        "let x = true",
        "let x = true // comment does not terminate a binding\n",
    ] {
        let source = model("Bool", body);
        let error = syntax::parse(&source).unwrap_err();
        assert!(error.message.contains("expected `;`"), "{body}: {error:?}");
        assert_eq!(error.span.start, error.span.end);
        assert!(source.is_char_boundary(error.span.start));
    }
    let source = "actor A { handle_message(m: unit): unit { send(A, ()) send(A, ()) } } property \"p\" { always true } check C { mailbox_bound = 2 }";
    assert!(
        syntax::parse(source)
            .unwrap_err()
            .message
            .contains("expected `;`")
    );
}

#[test]
fn only_unterminated_tails_return_a_value() {
    for (body, expected) in [
        ("let x = true; x", Value::Bool(true)),
        ("false; true", Value::Bool(true)),
        (
            "match Some(true) { | Some(x) -> x | None -> false }",
            Value::Bool(true),
        ),
    ] {
        let program = compile(&model("Bool", body), None).unwrap();
        assert_eq!(program.initial().unwrap().actors["A"], expected);
    }
    for body in [
        "",
        "true;",
        "let x = true;",
        "match Some(true) { | Some(x) -> x | None -> () };",
        "let x = true; // discarded final binding\n",
    ] {
        let program = compile(&model("unit", body), None).unwrap();
        assert_eq!(
            program.initial().unwrap().actors["A"],
            Value::Unit,
            "{body}"
        );
    }
    for body in [
        "true;",
        "match Some(true) { | Some(x) -> x | None -> false };",
    ] {
        assert!(
            compile(&model("Bool", body), None)
                .unwrap_err()
                .message
                .contains("type mismatch")
        );
    }
}

#[test]
fn terminated_results_cannot_bypass_error_handling() {
    for body in [
        "Ok(true);",
        "match Some(true) { | Some(x) -> Ok(x) | None -> Err(false) };",
        "let result = Ok(true);",
    ] {
        assert!(
            compile(&model("unit", body), None)
                .unwrap_err()
                .message
                .contains("Result"),
            "{body}"
        );
    }
}

#[test]
fn match_arms_use_bars_and_braced_statement_sequences() {
    let source = model(
        "Bool",
        "match Some(true) { | Some(x) -> { let y = x; false; y } | None -> false }",
    );
    let program = compile(&source, None).unwrap();
    assert_eq!(program.initial().unwrap().actors["A"], Value::Bool(true));
    for body in [
        "match Some(true) { | Some(x) -> x; | None -> false }",
        "match Some(true) { | Some(x) -> x None -> false }",
        "match Some(true) { | Some(x) -> let y = x | None -> false }",
        "; true",
        "true;;",
    ] {
        assert!(syntax::parse(&model("Bool", body)).is_err(), "{body}");
    }
}

#[test]
fn non_tail_match_requires_a_semicolon_and_discards_branch_values() {
    let program = compile(
        &model(
            "Bool",
            "match Some(true) { | Some(x) -> x | None -> () }; false",
        ),
        None,
    )
    .unwrap();
    assert_eq!(program.initial().unwrap().actors["A"], Value::Bool(false));
}

#[test]
fn send_then_parenthesized_value_is_not_a_call_on_send_result() {
    let source = r#"
actor A {
  init(): Bool { false }
  handle_message(s: Bool, m: unit): Bool {
    let next = !s;
    send(B, ()); // The following parentheses start the block's value.
    (next)
  }
}
actor B { handle_message(m: unit): unit { (); } }
property "turn commits" { reachable A.state }
property "delivery progresses" { forall (i in inputs(A)) { i.submitted leads_to i.processed } }
check C { mailbox_bound = 1 inputs { once send(A, ()) } fairness { weak runtime.progress } }
"#;
    let program = compile(source, None).unwrap();
    let report = checker::check(source, &program, &Options::default()).unwrap();
    assert_eq!(report.status, Status::VerifiedInScope);
    assert_eq!(report.claims[0].result, "REACHED");
    report
        .witness()
        .unwrap()
        .validate(source, &program)
        .unwrap();
    assert!(compile(&source.replace("send(B, ());", "send(B, ())"), None).is_err());
}

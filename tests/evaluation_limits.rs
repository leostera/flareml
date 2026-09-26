use flareml::{checker, compile};
#[test]
fn combinatorial_state_predicates_have_a_work_limit_and_release_it() {
    let mut predicate = "true".to_owned();
    for i in 0..22 {
        predicate = format!("forall (x{i} in Bool) {{ {predicate} }}");
    }
    let source =
        format!("property \"bounded\" {{ always ({predicate}) }} check C {{ mailbox_bound = 1 }}");
    let p = compile(&source, None).unwrap();
    let error = checker::check(&source, &p, &Default::default()).unwrap_err();
    assert!(error.message.contains("LIMIT: local evaluation"), "{error}");
    let good = "property \"ok\" { always true } check C { mailbox_bound = 1 }";
    let r = checker::check(good, &compile(good, None).unwrap(), &Default::default()).unwrap();
    assert_eq!(r.status, checker::Status::VerifiedInScope);
}
#[test]
fn temporal_expansion_is_bounded_even_when_inner_quantifiers_are_empty() {
    let mut formula = "forall (i in inputs(A)) { eventually i.processed }".to_owned();
    for i in 0..20 {
        formula = format!("forall (x{i} in Bool) {{ {formula} }}");
    }
    let source = format!(
        "actor A {{ handle_message(msg: unit): unit {{ () }} }} property \"bounded\" {{ {formula} }} check C {{ mailbox_bound = 1 }}"
    );
    let p = compile(&source, None).unwrap();
    let error = checker::check(&source, &p, &Default::default()).unwrap_err();
    assert!(
        error.message.contains("LIMIT: temporal expansion"),
        "{error}"
    );
}
#[test]
fn negative_integer_pool_literals_are_supported_without_unchecked_arithmetic() {
    let source = "actor A { init(): Int { -1 } handle_message(state: Int, message: unit): Int { state } } property \"negative\" { always (A.state == -1) } check C { mailbox_bound = 1 domain Int = [-1, 0, 1] }";
    let p = compile(source, None).unwrap();
    let r = checker::check(source, &p, &Default::default()).unwrap();
    assert_eq!(r.status, checker::Status::VerifiedInScope);
}

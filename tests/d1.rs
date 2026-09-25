use flareml::{
    checker::{self, Options, Status},
    compile,
};
fn model(operation: &str, initial: &str, predicate: &str) -> String {
    format!(
        r#"
type Id = A | B
type Reply = Accepted | Rejected
d1 DB {{ table Row {{ id: Id primary_key label: String unique }} }}
worker API {{ change(request: Id): Reply {{
 let result = {operation};
 match result {{ | Ok(_) -> respond(Accepted) | Err(_) -> respond(Rejected) }}
}} }}
invariant "rows" {{ {predicate} }}
property "response" {{ forall (r in requests(API.change)) {{ r.accepted leads_to r.completed }} }}
check C {{
 semantics = "cf-core-v0"
 domain String = ["x", "y"]
 init {{ DB.Row = {initial} }}
 inputs {{ once API.change(A) }}
 fairness {{ weak runtime.progress }}
}}
"#
    )
}
#[test]
fn rejected_mutations_leave_rows_unchanged() {
    for op in [
        "DB.Row.insert(Row { id: A, label: \"y\" })",
        "DB.Row.insert(Row { id: B, label: \"x\" })",
        "DB.Row.update(B, Row { id: B, label: \"y\" })",
        "DB.Row.delete(B)",
    ] {
        let src = model(
            op,
            "[Row { id: A, label: \"x\" }]",
            "DB.Row.rows.contains_key(A) && not DB.Row.rows.contains_key(B)",
        );
        let p = compile(&src, None).unwrap();
        let r = checker::check(&src, &p, &Options::default()).unwrap();
        assert_eq!(r.status, Status::VerifiedInScope, "{op}");
    }
}
#[test]
fn writes_commit_before_handler_responds() {
    for op in [
        "DB.Row.insert(Row { id: B, label: \"y\" })",
        "DB.Row.update(A, Row { id: B, label: \"y\" })",
        "DB.Row.delete(A)",
    ] {
        let src = model(
            op,
            "[Row { id: A, label: \"x\" }]",
            "DB.Row.rows.contains_key(A) && not DB.Row.rows.contains_key(B)",
        );
        let p = compile(&src, None).unwrap();
        let r = checker::check(&src, &p, &Options::default()).unwrap();
        assert_eq!(r.status, Status::Violated, "{op}");
        assert!(
            r.witness()
                .unwrap()
                .actions
                .last()
                .unwrap()
                .description
                .contains("completes")
        );
    }
}
#[test]
fn unsupported_sql_and_batch_are_not_accepted() {
    for op in [
        "DB.Row.query(\"x\")",
        "DB.Row.transaction(A)",
        "DB.Row.insert(A)",
    ] {
        assert!(compile(&model(op, "[]", "true"), None).is_err());
    }
}

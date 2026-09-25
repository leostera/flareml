use flareml::{
    checker::{self, Options, Status},
    compile,
};
use proptest::prelude::*;
const FIXED: &str = include_str!("../examples/login-fixed.fml");
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_source_does_not_panic(source in ".{0,1000}") { let _=compile(&source,None); }
    #[test]
    fn arbitrary_trace_json_does_not_panic(source in ".{0,1000}") { let _=serde_json::from_str::<flareml::trace::Trace>(&source); }
}
#[test]
fn missing_result_handling_is_rejected() {
    let src=FIXED.replace("let user = AppDB.User.get(request.user_id);","let ignored = AppDB.User.insert(User { id: request.user_id });\nlet user = AppDB.User.get(request.user_id);");
    assert!(
        compile(&src, None)
            .unwrap_err()
            .message
            .contains("Result binding")
    );
}
#[test]
fn nested_matches_hit_parser_guard() {
    let mut body = "respond(())".to_owned();
    for _ in 0..150 {
        body = format!("match request {{ | _ -> {body} }}");
    }
    let src = format!(
        "worker API {{ go(request: Bool) {{ {body} }} }} invariant \"ok\" {{ true }} check C {{ semantics = \"cf-core-v0\" }}"
    );
    assert!(compile(&src, None).unwrap_err().message.contains("nesting"));
}
#[test]
fn nullable_unique_fields_match_sqlite_null_behavior() {
    let src = r#"
type Id = A | B
d1 DB { table User { id: Id primary_key email: Option<String> unique } }
invariant "valid" { true }
check C { semantics = "cf-core-v0" domain String = ["shared"] init { DB.User = [User { id: A, email: None }, User { id: B, email: None }] } }
"#;
    let p = compile(src, None).unwrap();
    assert_eq!(
        checker::check(src, &p, &Options::default()).unwrap().status,
        Status::VerifiedInScope
    );
    let invalid = src.replace("email: None", "email: Some(\"shared\")");
    let p = compile(&invalid, None).unwrap();
    assert!(
        checker::check(&invalid, &p, &Options::default())
            .unwrap_err()
            .message
            .contains("initial constraint")
    );
}
#[test]
fn duplicate_check_and_claim_names_are_rejected() {
    assert!(
        compile(
            &format!("{FIXED}\ncheck Login {{ semantics = \"cf-core-v0\" }}"),
            Some("Login")
        )
        .is_err()
    );
    assert!(
        compile(
            &format!("{FIXED}\ninvariant \"a non-existing user can't log in\" {{ true }}"),
            None
        )
        .is_err()
    );
}
#[test]
fn model_types_cannot_be_cyclic_aliases() {
    let src = format!("type X = Y\ntype Y = X\n{FIXED}");
    assert!(compile(&src, None).unwrap_err().message.contains("cyclic"));
}
#[test]
fn keyword_singleton_constructor_is_not_an_alias_cycle() {
    let src = "type Tick = Tick invariant \"all ticks\" { forall (t in Tick) { t == Tick } } check C { semantics = \"cf-core-v0\" }";
    // Same-named constructors and type-domain references are currently ambiguous: reject
    // the collection use rather than claiming a vacuous check.
    assert!(compile(src, None).is_err());
}

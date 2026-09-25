#[test]
fn empty_payload_types_are_unified_across_collection_elements() {
    let invalid = r#"
invariant "typed" { forall (x in [None, Some(true), Some(1)]) { true } }
check C { semantics = "cf-core-v0" }
"#;
    assert!(
        flareml::compile(invalid, None)
            .unwrap_err()
            .message
            .contains("single compatible type")
    );
    let valid = invalid.replace("Some(1)", "Some(false)");
    assert!(flareml::compile(&valid, None).is_ok());
}

#[test]
fn record_patterns_do_not_reach_an_unimplemented_runtime_branch() {
    let source = r#"
type Input = Input { valid: Bool }
worker API { handle(request: Input): Bool { match request { | Input -> respond(true) } } }
invariant "ok" { true }
check C { semantics = "cf-core-v0" inputs { once API.handle(Input { valid: true }) } }
"#;
    assert!(
        flareml::compile(source, None)
            .unwrap_err()
            .message
            .contains("record constructor patterns")
    );
}

#[test]
fn result_wildcards_cannot_silently_discard_errors() {
    let source = r#"
type Id = A
d1 DB { table Item { id: Id primary_key } }
worker API { handle(request: Id): Bool {
  let result = DB.Item.insert(Item { id: request });
  match result { | _ -> respond(true) }
} }
invariant "ok" { true }
check C { semantics = "cf-core-v0" inputs { once API.handle(A) } }
"#;
    assert!(
        flareml::compile(source, None)
            .unwrap_err()
            .message
            .contains("Ok and Err")
    );
}

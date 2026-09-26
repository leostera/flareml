use flareml::{
    checker::{self, Options, Status},
    compile,
};

const BASE: &str = r#"
type Msg = Ping
actor A {
  init(): Bool { false }
  handle_message(state: Bool, msg: Msg): Bool { true }
}
invariant "state has a Boolean type" { A.state || !A.state }
check C { semantics = "actors-v2" mailbox_bound = 1 inputs { once send(A, Ping) } fairness { weak runtime.progress } }
"#;

#[test]
fn recursive_or_deep_data_and_call_graphs_fail_before_execution() {
    let recursive = format!("type Recursive = Leaf | Node(Recursive)\n{BASE}");
    assert!(
        compile(&recursive, None)
            .unwrap_err()
            .message
            .contains("recursive data")
    );
    let mut chain = String::from("let f0 = (x: Bool): Bool { x }\n");
    for i in 1..70 {
        chain.push_str(&format!("let f{i} = (x: Bool): Bool {{ f{}(x) }}\n", i - 1));
    }
    chain.push_str(BASE);
    assert!(
        compile(&chain, None)
            .unwrap_err()
            .message
            .contains("call depth")
    );
}

#[test]
fn pratt_chains_and_combined_helper_nesting_are_bounded() {
    let deep = BASE.replace("A.state || !A.state", &vec!["true"; 5000].join(" && "));
    assert!(
        compile(&deep, None)
            .unwrap_err()
            .message
            .contains("nesting")
    );
    let mut helpers = String::from("let f0 = (x: Bool): Bool { x }\n");
    for i in 1..40 {
        helpers.push_str(&format!(
            "let f{i} = (x: Bool): Bool {{ {}f{}(x) }}\n",
            "!".repeat(8),
            i - 1
        ));
    }
    let source = format!("{helpers}{}", BASE.replace("{ true }", "{ f39(true) }"));
    let p = compile(&source, None).unwrap();
    let report = checker::check(&source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert!(report.cutoff.unwrap().contains("evaluation nesting"));
    // A failed evaluation must release its host-stack budget.
    let p = compile(BASE, None).unwrap();
    assert_eq!(
        checker::check(BASE, &p, &Options::default())
            .unwrap()
            .status,
        Status::VerifiedInScope
    );
}

#[test]
fn v2_does_not_allow_owner_capabilities_or_inspection_to_escape() {
    for declaration in [
        "type Leak = Leak(Actor<Bool>)",
        "type Leak = Leak { hidden: Option<Actor<Bool>> }",
        "let steal = (owner: Actor<Bool>): Bool { owner.state }",
        "let leak = (x: Bool): Actor<Bool> { x }",
    ] {
        assert!(compile(&format!("{declaration}\n{BASE}"), None).is_err());
    }
    for body in [
        "let x = A.state; true",
        "let x = inputs(A); true",
        "let x = Some(send(A, msg)); true",
        "let x = [send(A, msg)]; true",
        "let x = call(A.handle_message, msg); true",
    ] {
        assert!(
            compile(&BASE.replace("{ true }", &format!("{{ {body} }}")), None).is_err(),
            "{body}"
        );
    }
}

#[test]
fn match_and_return_typing_are_exhaustive() {
    for source in [
        BASE.replace("Msg = Ping", "Msg = Ping | Pong")
            .replace("{ true }", "{ match msg { | Ping -> true } }"),
        BASE.replace("{ true }", "{ let next = true }"),
        BASE.replace("{ true }", "{ match msg { | Ping -> () } }"),
        BASE.replace("init(): Bool { false }", "init(): Bool { A.state }"),
        BASE.replace("init(): Bool { false }", "init(args: Bool): Bool { args }"),
    ] {
        assert!(compile(&source, None).is_err(), "{source}");
    }
}

#[test]
fn pure_helpers_cannot_hide_out_of_domain_intermediate_values() {
    let source = format!(
        "let helper = (x: Int): Bool {{ let outside = x + 1; true }}\n{}",
        BASE.replace("{ true }", "{ helper(1) }")
            .replace("mailbox_bound = 1", "mailbox_bound = 1 domain Int = 0..1")
    );
    let p = compile(&source, None).unwrap();
    let report = checker::check(&source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::Inconclusive);
    assert!(report.cutoff.unwrap().contains("escapes domain"));
}

#[test]
fn keyed_state_requires_an_address_and_init_aliases_are_transparent() {
    let source = BASE
        .replace("actor A {", "type Key = Int\nactor A(id: Key) {")
        .replace("init(): Bool", "init(key: Int): Bool")
        .replace("A.state", "A.at(0).state")
        .replace("send(A, Ping)", "send(A.at(0), Ping)")
        .replace("mailbox_bound = 1", "mailbox_bound = 1 domain Int = 0..1");
    let p = compile(&source, None).unwrap();
    assert_eq!(p.initial().unwrap().keyed_actors["A"].len(), 2);
    assert!(
        compile(&source.replace("A.at(0).state", "A.state"), None)
            .unwrap_err()
            .message
            .contains("keyed state")
    );
}

#[test]
fn address_initialization_is_declaration_order_independent_and_enumerable() {
    let source = r#"
type Key = Left | Right
type Msg = Ping
type Routes = Route(Address<Z>)
actor A {
  init(): Address<Z> { Z.at(Right) }
  handle_message(state: Address<Z>, msg: Msg): Address<Z> { send(state, msg) state }
}
actor Z(id: Key) {
  init(key: Key): Bool { false }
  handle_message(state: Bool, msg: Msg): Bool { true }
}
property "input is processed" { forall (i in inputs(A)) { i.submitted leads_to Z.at(Right).state } }
invariant "left remains isolated" { !Z.at(Left).state }
cover "address-bearing domain is finite" { exists (r in Routes) { r == Route(Z.at(Right)) } }
check C { semantics = "actors-v2" mailbox_bound = 1 inputs { once send(A, Ping) } fairness { weak runtime.progress } }
"#;
    let p = compile(source, None).unwrap();
    let report = checker::check(source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::VerifiedInScope);
}

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
property "state has a Boolean type" { always (forall (a in instances(A)) { a.state == Some(true) || a.state == Some(false) }) }
check C { spawn_bound A = 1 mailbox_bound = 1 main { let a = spawn(A); inputs { once send(a, Ping) } } fairness { weak runtime.progress } }
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
    let deep = BASE.replace(
        "a.state == Some(true) || a.state == Some(false)",
        &vec!["true"; 5000].join(" && "),
    );
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
fn behavior_cannot_access_state_capabilities_or_inspection() {
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
        BASE.replace("{ true }", "{ let next = true; }"),
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
fn initializer_aliases_are_transparent_but_definitions_are_not_references() {
    let source = BASE
        .replace("actor A {", "type Key = Int\nactor A {")
        .replace("init(): Bool", "init(key: Key): Bool")
        .replace("spawn(A)", "spawn(A, 0)")
        .replace("mailbox_bound = 1", "mailbox_bound = 1 domain Int = 0..1");
    let p = compile(&source, None).unwrap();
    assert_eq!(p.initial().unwrap().spawned["A"].len(), 1);
    assert!(compile(&source.replace("send(a, Ping)", "send(A, Ping)"), None).is_err());
}

#[test]
fn address_initialization_is_explicit_and_declaration_order_independent() {
    let source = r#"
type Key = Left | Right
type Msg = Ping
type Routes = Route(Actor<Z>)
actor A {
  init(target: Actor<Z>): Actor<Z> { target }
  handle_message(state: Actor<Z>, msg: Msg): Actor<Z> { send(state, msg); state }
}
actor Z {
  init(): Bool { false }
  handle_message(state: Bool, msg: Msg): Bool { true }
}
property "input is processed" { forall (i in inputs(A)) { i.submitted leads_to (exists (z in instances(Z)) { z.state == Some(true) }) } }
property "unreferenced instances remain isolated" {
  always (forall (z in instances(Z)) { (forall (a in instances(A)) { a.state != z.reference }) implies z.state == Some(false) })
}
check C { spawn_bound A = 1 spawn_bound Z = 2 mailbox_bound = 1 main { spawn(Z); let right = spawn(Z); let a = spawn(A, right); inputs { once send(a, Ping) } } fairness { weak runtime.progress } }
"#;
    let p = compile(source, None).unwrap();
    let report = checker::check(source, &p, &Options::default()).unwrap();
    assert_eq!(report.status, Status::VerifiedInScope);
}

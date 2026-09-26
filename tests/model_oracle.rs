//! Independent, exhaustive end-to-end oracle for Boolean, self-messaging actors.
//! With fairness, an accepted input follows a deterministic cycle; without it,
//! any reached value may stutter forever. Input submission is always optional.
use flareml::{
    checker::{self, Options},
    compile,
};

#[test]
fn boolean_transition_tables_temporal_classification_and_replay() {
    for table in 0..4 {
        let next = [table & 1 != 0, table & 2 != 0];
        let mut orbit = vec![false];
        let mut value = next[0];
        while !orbit.contains(&value) {
            orbit.push(value);
            value = next[value as usize];
        }
        let cycle = &orbit[orbit.iter().position(|v| *v == value).unwrap()..];
        for fair in [false, true] {
            for mask in 0..4 {
                let predicate = |v: bool| mask & (1 << v as usize) != 0;
                let initial = predicate(false);
                let all = orbit.iter().all(|v| predicate(*v));
                let some = orbit.iter().any(|v| predicate(*v));
                let recurrent = if fair {
                    initial && cycle.iter().any(|v| predicate(*v))
                } else {
                    all
                };
                let stable = if fair {
                    initial && cycle.iter().all(|v| predicate(*v))
                } else {
                    all
                };
                let p = format!(
                    "((!A.state && {}) || (A.state && {}))",
                    predicate(false),
                    predicate(true)
                );
                for (formula, expected) in [
                    (
                        format!("always {p}"),
                        if all { "VERIFIED" } else { "VIOLATED" },
                    ),
                    (
                        format!("eventually {p}"),
                        if initial { "VERIFIED" } else { "VIOLATED" },
                    ),
                    (
                        format!("always eventually {p}"),
                        if recurrent { "VERIFIED" } else { "VIOLATED" },
                    ),
                    (
                        format!("eventually always {p}"),
                        if stable { "VERIFIED" } else { "VIOLATED" },
                    ),
                    (
                        format!("reachable {p}"),
                        if some { "REACHED" } else { "UNREACHABLE" },
                    ),
                ] {
                    let source = format!(
                        "actor A {{ init(): Bool {{ false }} handle_message(s: Bool, m: unit): Bool {{ let result = ((s && {}) || (!s && {})) send(A, ()) result }} }} property \"p\" {{ {formula} }} check C {{ mailbox_bound = 1 inputs {{ once send(A, ()) }} {} }}",
                        next[1],
                        next[0],
                        if fair {
                            "fairness { weak runtime.progress }"
                        } else {
                            ""
                        }
                    );
                    let program = compile(&source, None).unwrap();
                    let report = checker::check(&source, &program, &Options::default()).unwrap();
                    let expected = if expected == "VERIFIED" {
                        "VERIFIED_IN_SCOPE"
                    } else {
                        expected
                    };
                    assert_eq!(report.claims[0].result, expected, "{source}");
                    if let Some(trace) = report.witness() {
                        trace.validate(&source, &program).unwrap();
                    }
                }
            }
        }
    }
}

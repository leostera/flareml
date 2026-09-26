//! Keep the scenario guide's inventory and advertised outcomes executable.
use flareml::{
    checker::{self, Options, Status},
    compile,
};
use std::{collections::BTreeSet, fs};

#[test]
fn scenario_examples_keep_their_profiles_verdicts_and_replay() {
    let scenarios = [
        ("counter-replies.fml", "actors-v2", Status::VerifiedInScope),
        ("missing-reply.fml", "actors-v2", Status::Violated),
        (
            "eligibility-check.fml",
            "actors-v0",
            Status::VerifiedInScope,
        ),
        ("counter-bound.fml", "actors-v0", Status::Violated),
        ("forwarded-counter.fml", "actors-v1", Status::Violated),
        (
            "isolated-accounts.fml",
            "actors-v1",
            Status::VerifiedInScope,
        ),
        ("routed-deposits.fml", "actors-v1", Status::VerifiedInScope),
        ("lost-update-across-call.fml", "actors-v1", Status::Violated),
        ("login-bug.fml", "cf-core-v0", Status::Violated),
        ("login-fixed.fml", "cf-core-v0", Status::VerifiedInScope),
        ("lost-update.fml", "cf-core-v0", Status::Violated),
        ("starvation.fml", "cf-core-v0", Status::Violated),
    ];
    let actual: BTreeSet<_> = fs::read_dir("examples")
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".fml"))
        .collect();
    let expected: BTreeSet<_> = scenarios
        .iter()
        .map(|(name, _, _)| name.to_string())
        .collect();
    assert_eq!(
        actual, expected,
        "update the scenario guide and acceptance matrix when adding examples"
    );
    let guide = fs::read_to_string("examples/README.md").unwrap();
    for (name, profile, status) in scenarios {
        assert!(
            guide.contains(&format!("]({name})")),
            "missing scenario guide entry: {name}"
        );
        let source = fs::read_to_string(format!("examples/{name}")).unwrap();
        let program = compile(&source, None).unwrap();
        assert_eq!(program.check.semantics, profile, "{name}");
        let report = checker::check(&source, &program, &Options::default()).unwrap();
        assert_eq!(report.status, status, "{name}");
        for claim in &report.claims {
            if let Some(witness) = &claim.witness {
                witness.validate(&source, &program).unwrap();
            }
        }
    }
}

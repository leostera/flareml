use flareml::{
    checker::{self, Options, Status},
    compile,
};
use std::collections::BTreeSet;
#[test]
fn every_scenario_has_a_checked_verdict_and_replay() {
    let fixtures = [
        ("explicit-startup.fml", Status::VerifiedInScope),
        ("counter-replies.fml", Status::VerifiedInScope),
        ("missing-reply.fml", Status::Violated),
        ("sequential-workflow.fml", Status::VerifiedInScope),
        ("lost-update.fml", Status::Violated),
        ("atomic-increments.fml", Status::VerifiedInScope),
        ("routed-deposits.fml", Status::VerifiedInScope),
        ("eligibility-check.fml", Status::VerifiedInScope),
        ("link-shortener.fml", Status::VerifiedInScope),
        ("inventory-reservation-bug.fml", Status::Violated),
        ("inventory-reservation-fixed.fml", Status::VerifiedInScope),
        ("payment-idempotency-bug.fml", Status::Violated),
        ("payment-idempotency-fixed.fml", Status::VerifiedInScope),
        ("spawn-workers.fml", Status::VerifiedInScope),
        ("spawn-choice-workers.fml", Status::VerifiedInScope),
        ("faulty-link-loss.fml", Status::Violated),
        ("faulty-link-duplicate-bug.fml", Status::Violated),
        ("faulty-link-duplicate-fixed.fml", Status::VerifiedInScope),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let actual: BTreeSet<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".fml"))
        .collect();
    assert_eq!(
        actual,
        fixtures.iter().map(|(n, _)| (*n).to_owned()).collect(),
        "new examples must declare an expected verdict"
    );
    for (name, expected) in fixtures {
        let source = std::fs::read_to_string(root.join(name)).unwrap();
        let p = compile(&source, None).unwrap();
        let r = checker::check(&source, &p, &Options::default()).unwrap();
        assert_eq!(r.status, expected, "{name}");
        for c in r.claims {
            if name.ends_with("-fixed.fml") && c.kind == flareml::syntax::ClaimKind::Cover {
                assert_eq!(c.result, "REACHED", "{name}: {}", c.name);
            }
            if let Some(trace) = c.witness {
                trace.validate(&source, &p).unwrap();
            }
        }
    }
}

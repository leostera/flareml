//! Keep the public guide and RFD's complete walkthrough executable.
use flareml::{
    checker::{self, Options, Status},
    compile,
};
#[test]
fn documented_language_walkthroughs_compile_check_and_replay() {
    for (name, markdown) in [
        ("README", include_str!("../README.md")),
        (
            "RFD0002",
            include_str!("../docs/rfds/RFD0002-functions-and-actors.md"),
        ),
    ] {
        let source = markdown
            .split("```fml\n")
            .skip(1)
            .map(|s| s.split("```").next().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!source.is_empty(), "{name} has no language example");
        let p = compile(&source, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        let r = checker::check(&source, &p, &Options::default()).unwrap();
        assert_eq!(r.status, Status::VerifiedInScope, "{name}");
        for c in r.claims {
            if let Some(trace) = c.witness {
                trace.validate(&source, &p).unwrap();
            }
        }
    }
}

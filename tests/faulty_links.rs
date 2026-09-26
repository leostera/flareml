use flareml::{
    checker::{self, Options, Status},
    compile,
};

#[test]
fn each_faulty_link_obligation_is_checked_independently() {
    let loss = include_str!("../examples/faulty-link-loss.fml");
    let bug = include_str!("../examples/faulty-link-duplicate-bug.fml");
    let fixed = include_str!("../examples/faulty-link-duplicate-fixed.fml");
    for (source, property, expected) in [
        (loss, "delivery is possible", "REACHED"),
        (loss, "submitted request eventually arrives", "VIOLATED"),
        (bug, "one application per request", "VIOLATED"),
        (bug, "delivery is possible", "REACHED"),
        (bug, "a duplicate can finish", "REACHED"),
        (fixed, "one application per request", "VERIFIED_IN_SCOPE"),
        (fixed, "delivery is possible", "REACHED"),
        (fixed, "a duplicate can finish", "REACHED"),
    ] {
        let p = compile(source, None).unwrap();
        let r = checker::check(
            source,
            &p,
            &Options {
                property: Some(property.into()),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.claims[0].result, expected, "{property}");
        assert_ne!(r.status, Status::Inconclusive);
        if let Some(trace) = r.witness() {
            trace.validate(source, &p).unwrap();
            if source == loss && expected == "VIOLATED" {
                assert!(trace.loop_start.is_some());
                assert!(
                    trace
                        .actions
                        .iter()
                        .flat_map(|a| &a.choices)
                        .any(|c| c.value
                            == flareml::semantics::Value::Variant("Drop".into(), vec![]))
                );
            }
        }
    }
}

#[test]
fn choice_witnesses_are_saved_and_replayed_through_the_cli() {
    let dir = tempfile::tempdir().unwrap();
    for model in [
        "faulty-link-loss.fml",
        "faulty-link-duplicate-bug.fml",
        "faulty-link-duplicate-fixed.fml",
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_fml"))
            .arg("check")
            .arg(format!("examples/{model}"))
            .args(["--format", "json", "--artifacts-dir"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(if model.ends_with("fixed.fml") { 0 } else { 1 })
        );
        let stdout: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let run = std::path::Path::new(stdout["artifacts_dir"].as_str().unwrap());
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("report.json")).unwrap()).unwrap();
        for witness in report["witness_files"].as_array().unwrap() {
            let path = run.join(witness["path"].as_str().unwrap());
            let mut trace: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let replay = || {
                std::process::Command::new(env!("CARGO_BIN_EXE_fml"))
                    .arg("replay")
                    .arg(run.join("model.fml"))
                    .arg(&path)
                    .output()
                    .unwrap()
            };
            let replayed = replay();
            assert!(replayed.status.success());
            assert!(String::from_utf8_lossy(&replayed.stdout).contains("choose #0"));
            let action = trace["actions"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|a| !a["choices"].as_array().unwrap().is_empty())
                .unwrap();
            action["choices"][0]["candidate"] = serde_json::json!(99);
            std::fs::write(&path, serde_json::to_vec(&trace).unwrap()).unwrap();
            assert_eq!(replay().status.code(), Some(4));
        }
    }
}

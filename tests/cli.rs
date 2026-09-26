use std::{
    fs,
    process::{Command, Output},
};
fn fml(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fml"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn check_exit_codes_and_json() {
    for (args, code, status) in [
        (
            vec!["check", "examples/counter-replies.fml", "--format", "json"],
            0,
            "VERIFIED_IN_SCOPE",
        ),
        (
            vec!["check", "examples/missing-reply.fml", "--format", "json"],
            1,
            "VIOLATED",
        ),
        (
            vec![
                "check",
                "examples/counter-replies.fml",
                "--max-states",
                "1",
                "--format",
                "json",
            ],
            3,
            "INCONCLUSIVE",
        ),
        (
            vec!["check", "/nonexistent-flareml-model", "--format", "json"],
            4,
            "TOOL_ERROR",
        ),
    ] {
        let out = fml(&args);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["status"], status);
        assert!(value.get("semantics").is_none());
    }
}
#[test]
fn invalid_source_is_structured_and_source_mapped() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("invalid.fml");
    fs::write(
        &file,
        "property \"broken\" {\n always missing_name\n}\ncheck C { mailbox_bound = 1 }",
    )
    .unwrap();
    let out = fml(&["check", file.to_str().unwrap(), "--format", "json"]);
    assert_eq!(out.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["status"], "INVALID_MODEL");
    assert!(json["error"]["span"]["start"].as_u64().unwrap() > 0);
}
#[test]
fn finite_and_lasso_artifacts_are_saved_and_replayed() {
    let dir = tempfile::tempdir().unwrap();
    for source in ["examples/lost-update.fml", "examples/missing-reply.fml"] {
        let trace = dir.path().join("counterexample.json");
        assert_eq!(
            fml(&["check", source, "--trace-out", trace.to_str().unwrap()])
                .status
                .code(),
            Some(1)
        );
        let out = fml(&[
            "replay",
            source,
            trace.to_str().unwrap(),
            "--format",
            "json",
        ]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json["status"], "REPLAY_VALIDATED");
        assert_eq!(
            fml(&[
                "replay",
                "examples/counter-replies.fml",
                trace.to_str().unwrap()
            ])
            .status
            .code(),
            Some(4)
        );
    }
}
#[test]
fn current_trace_format_only_and_no_semantics_switch() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("keyed.json");
    let out = fml(&[
        "check",
        "examples/lost-update.fml",
        "--format",
        "json",
        "--trace-out",
        trace.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let artifact: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert_eq!(artifact["format_version"], flareml::trace::FORMAT_VERSION);
    assert!(artifact.get("semantics").is_none());
    assert!(artifact["states"][0]["keyed_actors"]["Client"].is_array());
    assert!(artifact["states"][0].get("frames").is_none());
    assert!(artifact["states"][0].get("tables").is_none());
    for version in [0, 1, 2, 3, 4, 5, 999] {
        let mut old = artifact.clone();
        old["format_version"] = version.into();
        fs::write(&trace, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(
            fml(&[
                "replay",
                "examples/lost-update.fml",
                trace.to_str().unwrap()
            ])
            .status
            .code(),
            Some(4)
        );
    }
}
#[test]
fn async_fair_and_unfair_progress_replays_through_cli() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("messages.fml");
    let trace = dir.path().join("messages.json");
    let good = include_str!("../examples/counter-replies.fml");
    for (text, code) in [
        (good.to_owned(), 0),
        (good.replace("fairness { weak runtime.progress }", ""), 1),
    ] {
        fs::write(&source, text).unwrap();
        let out = fml(&[
            "--color",
            "always",
            "check",
            source.to_str().unwrap(),
            "--format",
            "json",
            "--trace-out",
            trace.to_str().unwrap(),
        ]);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(!out.stdout.contains(&0x1b));
        let artifact: serde_json::Value =
            serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
        assert_eq!(artifact["message_bound"], 2);
        if code == 1 {
            assert!(artifact["loop_start"].is_number());
        }
        assert_eq!(
            fml(&["replay", source.to_str().unwrap(), trace.to_str().unwrap()])
                .status
                .code(),
            Some(0)
        );
    }
}
#[test]
fn reachability_and_property_selection_have_explicit_exit_policy() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("properties.fml");
    fs::write(&source, "property \"safe\" { always false } property \"possible\" { reachable false } check C { mailbox_bound = 1 }").unwrap();
    let out = fml(&[
        "check",
        source.to_str().unwrap(),
        "--property",
        "possible",
        "--format",
        "json",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["claims"][0]["result"], "UNREACHABLE");
    assert_eq!(r["not_checked"][0], "safe");
    assert_eq!(
        fml(&["check", source.to_str().unwrap(), "--property", "safe"])
            .status
            .code(),
        Some(1)
    );
}
#[test]
fn trace_output_cannot_overwrite_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("model.fml");
    let text = include_str!("../examples/missing-reply.fml");
    fs::write(&source, text).unwrap();
    assert_eq!(
        fml(&[
            "check",
            source.to_str().unwrap(),
            "--trace-out",
            source.to_str().unwrap()
        ])
        .status
        .code(),
        Some(4)
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), text);
}
#[test]
fn malformed_trace_is_tool_error() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("bad.json");
    fs::write(&trace, "{}").unwrap();
    assert_eq!(
        fml(&[
            "replay",
            "examples/counter-replies.fml",
            trace.to_str().unwrap(),
            "--format",
            "json"
        ])
        .status
        .code(),
        Some(4)
    );
}
#[test]
fn version_and_help() {
    assert!(fml(&["--version"]).status.success());
    assert!(fml(&["--help"]).status.success());
}

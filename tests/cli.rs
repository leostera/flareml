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
            vec!["check", "examples/login-fixed.fml", "--format", "json"],
            0,
            "VERIFIED_IN_SCOPE",
        ),
        (
            vec!["check", "examples/login-bug.fml", "--format", "json"],
            1,
            "VIOLATED",
        ),
        (
            vec![
                "check",
                "examples/login-fixed.fml",
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
    }
}
#[test]
fn invalid_source_is_structured_and_source_mapped() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("invalid.fml");
    fs::write(
        &file,
        "invariant \"broken\" {\n missing_name\n}\ncheck C { semantics = \"cf-core-v0\" }",
    )
    .unwrap();
    let out = fml(&["check", file.to_str().unwrap(), "--format", "json"]);
    assert_eq!(out.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["status"], "INVALID_MODEL");
    assert!(json["error"]["span"]["start"].as_u64().unwrap() > 0);
}
#[test]
fn artifact_is_saved_and_replayed() {
    let dir = tempfile::tempdir().unwrap();
    for source in ["examples/login-bug.fml", "examples/starvation.fml"] {
        let trace = dir.path().join("counterexample.json");
        let out = fml(&["check", source, "--trace-out", trace.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1));
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
            String::from_utf8_lossy(&out.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json["status"], "REPLAY_VALIDATED");
        let out = fml(&[
            "replay",
            "examples/login-fixed.fml",
            trace.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(4));
    }
}
#[test]
fn keyed_actor_trace_roundtrips_through_cli_json() {
    let dir = tempfile::tempdir().unwrap();
    let source = "examples/actor-interleaving.fml";
    let trace = dir.path().join("keyed.json");
    let check = fml(&[
        "check",
        source,
        "--format",
        "json",
        "--trace-out",
        trace.to_str().unwrap(),
    ]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(report["semantics"], "actors-v1");
    let artifact: serde_json::Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
    assert_eq!(artifact["format_version"], 3);
    assert!(artifact["states"][0]["keyed_actors"]["Counter"].is_array());
    let replay = fml(&[
        "replay",
        source,
        trace.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(replay.status.code(), Some(0));
    let result: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(result["status"], "REPLAY_VALIDATED");
}
#[test]
fn malformed_trace_is_tool_error() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("bad.json");
    fs::write(&trace, "{}").unwrap();
    let out = fml(&[
        "replay",
        "examples/login-fixed.fml",
        trace.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(out.status.code(), Some(4));
}
#[test]
fn lost_update_is_found_through_public_cli() {
    let out = fml(&["check", "examples/lost-update.fml", "--format", "json"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["claims"][0]["result"], "VIOLATED");
}
#[test]
fn version_and_help() {
    assert!(fml(&["--version"]).status.success());
    assert!(fml(&["--help"]).status.success());
}

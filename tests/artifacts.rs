use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn check(source: &Path, root: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fml"))
        .arg("check")
        .arg(source)
        .arg("--artifacts-dir")
        .arg(root)
        .args(["--format", "json"])
        .args(extra)
        .output()
        .unwrap()
}
fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn replay(run: &Path, witness: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fml"))
        .arg("replay")
        .arg(run.join("model.fml"))
        .arg(witness)
        .output()
        .unwrap()
}
#[test]
fn saves_every_witness_with_safe_filenames_and_replays_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.fml");
    let text = "property \"../../escaped\" { reachable true } property \"second\" { reachable true } property \"safe\" { always true } check C { mailbox_bound = 1 }";
    fs::write(&source, text).unwrap();
    let root = dir.path().join("runs");
    let out = check(&source, &root, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout: Value = serde_json::from_slice(&out.stdout).unwrap();
    let run = Path::new(stdout["artifacts_dir"].as_str().unwrap());
    let report = json(&run.join("report.json"));
    assert_eq!(report["status"], stdout["status"]);
    assert_eq!(report["complete"], true);
    assert_eq!(report["witness_files"].as_array().unwrap().len(), 2);
    assert!(report["claims"][2]["witness"].is_null());
    assert_eq!(
        json(&run.join("configuration.json"))["source_sha256"],
        flareml::trace::source_hash(text)
    );
    fs::write(&source, "changed after run").unwrap();
    assert_eq!(fs::read_to_string(run.join("model.fml")).unwrap(), text);
    for witness in report["witness_files"].as_array().unwrap() {
        let path = run.join(witness["path"].as_str().unwrap());
        assert!(replay(run, &path).status.success());
        let mut corrupt = json(&path);
        corrupt["source_hash"] = Value::String("corrupted".into());
        fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        assert_eq!(replay(run, &path).status.code(), Some(4));
    }
    assert!(!dir.path().join("escaped").exists());
}
#[test]
fn persists_incomplete_and_invalid_runs_without_fabricating_witnesses() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("runs");
    let source = dir.path().join("model.fml");
    fs::write(&source, include_str!("../examples/counter-replies.fml")).unwrap();
    let out = check(&source, &root, &["--max-states", "1"]);
    assert_eq!(out.status.code(), Some(3));
    let stdout: Value = serde_json::from_slice(&out.stdout).unwrap();
    let run = Path::new(stdout["artifacts_dir"].as_str().unwrap());
    let report = json(&run.join("report.json"));
    assert_eq!(report["complete"], false);
    assert!(report["cutoff"].is_string());
    assert_eq!(report["witness_files"], serde_json::json!([]));
    fs::write(&source, "not a model").unwrap();
    assert_eq!(check(&source, &root, &[]).status.code(), Some(2));
    let reports: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| json(&entry.unwrap().path().join("report.json")))
        .collect();
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().any(|r| r["status"] == "INVALID_MODEL"));
}
#[test]
fn persistence_failure_is_not_a_successful_verification() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("not-a-directory");
    fs::write(&root, "keep").unwrap();
    let out = check(Path::new("examples/counter-replies.fml"), &root, &[]);
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["status"],
        "TOOL_ERROR"
    );
    assert_eq!(fs::read_to_string(root).unwrap(), "keep");
}
#[test]
fn default_bundles_replay_finite_and_lasso_counterexamples() {
    for (name, lasso) in [("lost-update.fml", false), ("missing-reply.fml", true)] {
        let dir = tempfile::tempdir().unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(name);
        let out = Command::new(env!("CARGO_BIN_EXE_fml"))
            .current_dir(dir.path())
            .arg("check")
            .arg(source)
            .args(["--format", "json"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let stdout: Value = serde_json::from_slice(&out.stdout).unwrap();
        let relative = Path::new(stdout["artifacts_dir"].as_str().unwrap());
        assert!(relative.starts_with(".fml/runs"));
        let run = dir.path().join(relative);
        let report = json(&run.join("report.json"));
        let witnesses = report["witness_files"].as_array().unwrap();
        assert!(!witnesses.is_empty());
        for witness in witnesses {
            let path = run.join(witness["path"].as_str().unwrap());
            assert!(replay(&run, &path).status.success());
            if witness["result"] == "VIOLATED" {
                let mut trace = json(&path);
                assert_eq!(trace["loop_start"].is_number(), lasso);
                trace["states"][0]["actors"] = serde_json::json!({});
                fs::write(&path, serde_json::to_vec(&trace).unwrap()).unwrap();
                assert_eq!(replay(&run, &path).status.code(), Some(4));
            }
        }
    }
}

#[test]
fn concurrent_runs_get_distinct_complete_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let mut children: Vec<_> = (0..4)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_fml"))
                .args(["check", "examples/eligibility-check.fml", "--artifacts-dir"])
                .arg(dir.path())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let runs: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(runs.len(), 4);
    for run in runs {
        assert_eq!(
            json(&run.unwrap().path().join("report.json"))["status"],
            "VERIFIED_IN_SCOPE"
        );
    }
}

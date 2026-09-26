use flareml::{
    checker::{self, Options},
    compile,
    semantics::Value,
};

#[test]
fn allocation_transcripts_registries_and_bounds_are_reexecuted_not_trusted() {
    let source = include_str!("../examples/spawn-choice-workers.fml");
    let p = compile(source, None).unwrap();
    let report = checker::check(source, &p, &Options::default()).unwrap();
    let trace = report.witness().unwrap();
    trace.validate(source, &p).unwrap();
    let at = trace
        .actions
        .iter()
        .position(|a| !a.spawns.is_empty())
        .unwrap();
    assert_eq!(trace.actions[at].spawns.len(), 2);
    assert_eq!(trace.actions[at].choices.len(), 1);
    for mutation in 0..13 {
        let mut bad = trace.clone();
        match mutation {
            0 => {
                bad.actions[at].spawns.pop();
            }
            1 => {
                let duplicate = bad.actions[at].spawns[0].clone();
                bad.actions[at].spawns.push(duplicate);
            }
            2 => bad.actions[at].spawns.swap(0, 1),
            3 => {
                bad.actions[at].spawns[0].address =
                    Value::Address("Worker".into(), Box::new(Value::Identity(99)))
            }
            4 => bad.actions[at].spawns[0].initial = Value::Bool(true),
            5 => bad.actions[at].spawns[0].span.start += 1,
            6 => bad.actions[at].spawns[0].calls.push(Default::default()),
            7 => {
                bad.states[at + 1].spawned.get_mut("Worker").unwrap().pop();
            }
            8 => {
                bad.states[0]
                    .spawned
                    .insert("Worker".into(), vec![Value::Bool(false)]);
            }
            9 => {
                bad.spawn_bounds.insert("Worker".into(), 3);
            }
            10 => {
                let key = Value::Address("Worker".into(), Box::new(Value::Identity(0)));
                bad.states[at + 1].mailboxes.remove(&key);
            }
            11 => bad.actions[at].spawns[0].arguments.push(Value::Bool(true)),
            _ => {
                bad.actions[at].choices[0].candidate = 0;
                bad.actions[at].choices[0].value = Value::Variant("One".into(), vec![]);
            }
        }
        assert!(bad.validate(source, &p).is_err(), "mutation {mutation}");
    }
    let mut json = serde_json::to_value(trace).unwrap();
    json["actions"][at]["spawns"][0]["untrusted"] = true.into();
    assert!(serde_json::from_value::<flareml::trace::Trace>(json).is_err());
}

#[test]
fn persisted_spawn_witness_replays_snapshot_and_displays_allocation() {
    let dir = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_fml"))
        .args([
            "check",
            "examples/spawn-workers.fml",
            "--format",
            "json",
            "--artifacts-dir",
        ])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["spawn_bounds"]["Worker"], 2);
    let run = std::path::Path::new(report["artifacts_dir"].as_str().unwrap());
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run.join("report.json")).unwrap()).unwrap();
    let witnesses = saved["witness_files"].as_array().unwrap();
    assert!(!witnesses.is_empty());
    for witness in witnesses {
        let path = run.join(witness["path"].as_str().unwrap());
        let replay = || {
            std::process::Command::new(env!("CARGO_BIN_EXE_fml"))
                .arg("replay")
                .arg(run.join("model.fml"))
                .arg(&path)
                .output()
                .unwrap()
        };
        let out = replay();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("spawn Worker[instance #0]"));
        let mut trace: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        trace["spawn_bounds"]["Worker"] = 1.into();
        std::fs::write(&path, serde_json::to_vec(&trace).unwrap()).unwrap();
        assert_eq!(replay().status.code(), Some(4));
    }
}

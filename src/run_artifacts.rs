//! CLI evidence bundles. report.json is written last, as the completion marker.
use flareml::checker::Report;
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Run {
    pub path: PathBuf,
}
impl Run {
    pub fn create(root: &Path, source: &str, configuration: Value) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        for serial in 0..1000 {
            let path = root.join(format!("{time}-{}-{serial}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::write(path.join("model.fml"), source)?;
                    write_json(
                        &path.join("configuration.json"),
                        &json!({
                            "format_version": 1,
                            "tool_version": env!("CARGO_PKG_VERSION"),
                            "source_sha256": flareml::trace::source_hash(source),
                            "options": configuration
                        }),
                    )?;
                    return Ok(Self { path });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other(
            "could not allocate a unique run directory",
        ))
    }
    pub fn report(&self, report: &Report) -> io::Result<()> {
        let mut witnesses = Vec::new();
        for (index, claim) in report.claims.iter().enumerate() {
            if let Some(trace) = &claim.witness {
                fs::create_dir_all(self.path.join("witnesses"))?;
                // Property names are untrusted text, not filesystem paths.
                let relative = format!("witnesses/{index:04}.json");
                write_json(&self.path.join(&relative), &serde_json::to_value(trace)?)?;
                witnesses
                    .push(json!({"claim": claim.name, "result": claim.result, "path": relative}));
            }
        }
        let mut value = serde_json::to_value(report)?;
        value["artifact_format_version"] = json!(1);
        value["witness_files"] = json!(witnesses);
        self.finish(&value)
    }
    pub fn error(&self, value: &Value) -> io::Result<()> {
        if self.path.join("report.json").exists() {
            write_json(&self.path.join("error.json"), value)
        } else {
            self.finish(value)
        }
    }
    pub fn finish(&self, value: &Value) -> io::Result<()> {
        write_json(&self.path.join("report.json.tmp"), value)?;
        fs::rename(
            self.path.join("report.json.tmp"),
            self.path.join("report.json"),
        )
    }
}
fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)
}

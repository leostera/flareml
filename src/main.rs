mod explorer_server;
mod run_artifacts;

use clap::{Parser, Subcommand, ValueEnum};
use flareml::{
    checker::{self, Options},
    diagnostics,
    syntax::{Error, Span},
    trace::Trace,
};
use std::{
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

#[derive(Parser)]
#[command(
    name = "fml",
    version,
    about = "Check finite system models with a native checker"
)]
struct Cli {
    #[arg(long, global = true, value_enum, default_value = "auto")]
    color: Color,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}
#[derive(Clone, Copy, ValueEnum)]
enum Color {
    Auto,
    Always,
    Never,
}
impl Color {
    fn enabled(self, stderr: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                if std::env::var_os("NO_COLOR").is_some() {
                    return false;
                }
                if stderr {
                    std::io::stderr().is_terminal()
                        && supports_color::on(supports_color::Stream::Stderr).is_some()
                } else {
                    std::io::stdout().is_terminal()
                        && supports_color::on(supports_color::Stream::Stdout).is_some()
                }
            }
        }
    }
}
#[derive(Subcommand)]
enum Command {
    /// Print the bundled agent guide or a detailed manual topic (Markdown).
    Skills {
        /// Install SKILL.md and all topic manuals into ~/.agents/skills/flareml/.
        #[arg(long)]
        install: bool,
        #[command(subcommand)]
        topic: Option<SkillTopic>,
    },
    /// Explore a finite model and check its selected properties.
    Check {
        /// Model source (.fml).
        model: PathBuf,
        /// Select a named check when the source declares more than one.
        #[arg(long = "check")]
        selected: Option<String>,
        /// Check only this named property (including reachability queries).
        #[arg(long)]
        property: Option<String>,
        #[arg(long, default_value_t = 100_000)]
        max_states: usize,
        #[arg(long, default_value_t = 1000)]
        max_depth: usize,
        #[arg(long, default_value="30s", value_parser=humantime::parse_duration)]
        timeout: Duration,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        /// Save a counterexample, or a reached witness if no violation was found.
        #[arg(long)]
        trace_out: Option<PathBuf>,
        /// Open the trace explorer if a property violation is found.
        #[arg(long, conflicts_with = "format")]
        ui: bool,
        /// Print the explorer URL without launching a browser.
        #[arg(long, requires = "ui")]
        no_open: bool,
        /// Parent directory for automatically saved, uniquely named run bundles.
        #[arg(long, default_value = ".fml/runs")]
        artifacts_dir: PathBuf,
    },
    /// Re-execute a current-format trace and validate its evidence.
    Replay {
        /// Model source, or a saved run directory containing model.fml and report.json.
        model: PathBuf,
        /// Trace JSON (omit when replaying a run directory).
        trace: Option<PathBuf>,
        /// Zero-based index in the run's saved witness list (defaults to the first).
        #[arg(long, conflicts_with = "trace")]
        witness: Option<usize>,
        /// Open the embedded, local interactive trace explorer.
        #[arg(long, conflicts_with = "format")]
        ui: bool,
        /// Print the explorer URL without launching a browser.
        #[arg(long, requires = "ui")]
        no_open: bool,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
}
#[derive(Subcommand)]
enum SkillTopic {
    /// Types, functions, expressions, patterns, and precedence.
    Syntax,
    /// Actor identity, messages, atomic turns, and modeling boundaries.
    Actors,
    /// Safety, reachability, temporal claims, and fairness.
    Properties,
    /// Finite experiments, bounds, inputs, and exploration.
    Checks,
    /// Read-only input and message histories in specifications.
    Observations,
    /// CLI usage, results, traces, and replay.
    Cli,
}
fn skill(topic: Option<&SkillTopic>) -> &'static str {
    match topic {
        None => include_str!("../docs/skills/fml/SKILL.md"),
        Some(SkillTopic::Syntax) => include_str!("../docs/skills/fml/syntax.md"),
        Some(SkillTopic::Actors) => include_str!("../docs/skills/fml/actors.md"),
        Some(SkillTopic::Properties) => include_str!("../docs/skills/fml/properties.md"),
        Some(SkillTopic::Checks) => include_str!("../docs/skills/fml/checks.md"),
        Some(SkillTopic::Observations) => include_str!("../docs/skills/fml/observations.md"),
        Some(SkillTopic::Cli) => include_str!("../docs/skills/fml/cli.md"),
    }
}
fn install_skills() -> std::result::Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()))
        .ok_or("HOME (or USERPROFILE) is not set")?;
    let dest = PathBuf::from(home).join(".agents/skills/flareml");
    let pages = [
        ("SKILL.md", skill(None)),
        ("syntax.md", skill(Some(&SkillTopic::Syntax))),
        ("actors.md", skill(Some(&SkillTopic::Actors))),
        ("properties.md", skill(Some(&SkillTopic::Properties))),
        ("checks.md", skill(Some(&SkillTopic::Checks))),
        ("observations.md", skill(Some(&SkillTopic::Observations))),
        ("cli.md", skill(Some(&SkillTopic::Cli))),
    ];
    match fs::symlink_metadata(&dest) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(format!("{} is not a regular directory", dest.display()));
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("{}: {e}", dest.display()));
        }
        _ => {}
    }
    // Check every destination before writing any file, to avoid overwriting user edits.
    for (name, content) in pages {
        let path = dest.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
                return Err(format!("{} is not a regular file", path.display()));
            }
            Ok(_) => {
                if fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?
                    != content.as_bytes()
                {
                    return Err(format!(
                        "{} differs from the bundled skill; back it up or remove it before installing",
                        path.display()
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    for (name, content) in pages {
        let path = dest.join(name);
        if !path.exists() {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            file.write_all(content.as_bytes())
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(dest)
}
fn read(path: &Path, max: u64) -> std::result::Result<String, String> {
    let size = fs::metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if size > max {
        return Err(format!("{} exceeds input size limit", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn error(format: Format, color: Color, status: &str, error: &Error, path: &Path, source: &str) {
    if matches!(format, Format::Json) {
        println!(
            "{}",
            serde_json::json!({"status":status,"error":error,"file":path})
        );
    } else {
        eprintln!(
            "{status}\n{}",
            diagnostics::render_error(
                error,
                &path.display().to_string(),
                source,
                color.enabled(true)
            )
        );
    }
}
fn replay_paths(
    model: &Path,
    trace: Option<&Path>,
    witness: Option<usize>,
) -> Result<(PathBuf, PathBuf), String> {
    if let Some(trace) = trace {
        return Ok((model.to_owned(), trace.to_owned()));
    }
    if !model.is_dir() {
        return Err("provide a run directory, or both a model file and a trace JSON file".into());
    }
    let report: serde_json::Value =
        serde_json::from_str(&read(&model.join("report.json"), 16_000_000)?)
            .map_err(|e| format!("invalid run report: {e}"))?;
    let entries = report["witness_files"]
        .as_array()
        .ok_or("run report has no saved witness list")?;
    if entries.is_empty() {
        return Err("this run has no saved witnesses to replay; verified claims do not have execution traces".into());
    }
    let index = witness.unwrap_or(0);
    let entry = entries.get(index).ok_or_else(|| {
        format!(
            "witness index {index} out of range; this run has {} saved witnesses (indices 0–{})",
            entries.len(),
            entries.len() - 1
        )
    })?;
    let relative = entry["path"]
        .as_str()
        .ok_or("invalid witness path in run report")?;
    let filename = relative
        .strip_prefix("witnesses/")
        .and_then(|s| s.strip_suffix(".json"))
        .filter(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        .ok_or("invalid witness path in run report; expected witnesses/<digits>.json")?;
    eprintln!(
        "Replaying witness {index} of {}: {} (use --witness N to select another)",
        entries.len(),
        entry["claim"]
    );
    Ok((
        model.join("model.fml"),
        model.join("witnesses").join(format!("{filename}.json")),
    ))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Command::Skills { install, topic } = &cli.command {
        if *install && topic.is_some() {
            eprintln!("--install cannot be combined with a skills topic");
            return ExitCode::from(2);
        }
        if *install {
            return match install_skills() {
                Ok(dest) => {
                    println!("Installed FlareML skills in {}", dest.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Could not install FlareML skills: {e}");
                    ExitCode::from(4)
                }
            };
        }
        print!("{}", skill(topic.as_ref()));
        return ExitCode::SUCCESS;
    }
    let (path, format) = match &cli.command {
        Command::Skills { .. } => unreachable!(),
        Command::Check { model, format, .. } | Command::Replay { model, format, .. } => {
            (model, *format)
        }
    };
    let replay = if let Command::Replay {
        model,
        trace,
        witness,
        ..
    } = &cli.command
    {
        match replay_paths(model, trace.as_deref(), *witness) {
            Ok(paths) => Some(paths),
            Err(message) => {
                error(
                    format,
                    cli.color,
                    "TOOL_ERROR",
                    &Error::new(Span::default(), message),
                    path,
                    "",
                );
                return ExitCode::from(4);
            }
        }
    } else {
        None
    };
    let path = replay.as_ref().map(|(model, _)| model).unwrap_or(path);
    let source = match read(path, 1_000_000) {
        Ok(s) => s,
        Err(e) => {
            error(
                format,
                cli.color,
                "TOOL_ERROR",
                &Error::new(Span::default(), e),
                path,
                "",
            );
            return ExitCode::from(4);
        }
    };
    let run = if let Command::Check {
        artifacts_dir,
        selected,
        property,
        max_states,
        max_depth,
        timeout,
        ..
    } = &cli.command
    {
        match run_artifacts::Run::create(
            artifacts_dir,
            &source,
            serde_json::json!({
                "model": path, "check": selected, "property": property,
                "max_states": max_states, "max_depth": max_depth,
                "timeout": { "seconds": timeout.as_secs(), "nanoseconds": timeout.subsec_nanos() }
            }),
        ) {
            Ok(run) => Some(run),
            Err(e) => {
                error(
                    format,
                    cli.color,
                    "TOOL_ERROR",
                    &Error::new(Span::default(), format!("cannot create run artifacts: {e}")),
                    path,
                    &source,
                );
                return ExitCode::from(4);
            }
        }
    } else {
        None
    };
    let fail = |status: &str, e: &Error, source: &str| {
        if let Some(run) = &run {
            if let Err(io) = run.error(&serde_json::json!({"artifact_format_version": 1, "status": status, "error": e, "file": path})) {
                error(format, cli.color, "TOOL_ERROR", &Error::new(Span::default(), format!("cannot save error report: {io}; original error: {}", e.message)), path, source);
                return ExitCode::from(4);
            }
            eprintln!("Artifacts: {}", run.path.display());
        }
        error(format, cli.color, status, e, path, source);
        ExitCode::from(match status {
            "INVALID_MODEL" => 2,
            "INCONCLUSIVE" => 3,
            _ => 4,
        })
    };
    match &cli.command {
        Command::Skills { .. } => unreachable!(),
        Command::Check {
            selected,
            property,
            max_states,
            max_depth,
            timeout,
            trace_out,
            ui,
            no_open,
            ..
        } => {
            let p = match flareml::compile(&source, selected.as_deref()) {
                Ok(p) => p,
                Err(e) => {
                    return fail("INVALID_MODEL", &e, &source);
                }
            };
            let options = Options {
                max_states: *max_states,
                max_depth: *max_depth,
                timeout: *timeout,
                property: property.clone(),
            };
            let report = match checker::check(&source, &p, &options) {
                Ok(r) => r,
                Err(e) => {
                    let internal = e.message.starts_with("internal:")
                        || e.message.starts_with("invalid trace:");
                    let limit = e.message.starts_with("LIMIT:");
                    let status = if internal {
                        "TOOL_ERROR"
                    } else if limit {
                        "INCONCLUSIVE"
                    } else {
                        "INVALID_MODEL"
                    };
                    return fail(status, &e, &source);
                }
            };
            let run = run.as_ref().expect("check run allocated");
            if let Err(e) = run.report(&report) {
                return fail(
                    "TOOL_ERROR",
                    &Error::new(Span::default(), format!("cannot save run report: {e}")),
                    &source,
                );
            }
            eprintln!("Artifacts: {}", run.path.display());
            if let Some(output) = trace_out
                && let Some(trace) = report.witness()
            {
                // Canonicalization also catches alternate spellings and existing symlinks.
                if fs::canonicalize(output)
                    .ok()
                    .zip(fs::canonicalize(path).ok())
                    .is_some_and(|(a, b)| a == b)
                {
                    fail(
                        "TOOL_ERROR",
                        &Error::new(
                            Span::default(),
                            "trace output cannot overwrite model source",
                        ),
                        &source,
                    );
                    return ExitCode::from(4);
                }
                let result = serde_json::to_string_pretty(trace)
                    .map_err(|e| e.to_string())
                    .and_then(|s| fs::write(output, s).map_err(|e| e.to_string()));
                if let Err(e) = result {
                    fail("TOOL_ERROR", &Error::new(Span::default(), e), &source);
                    return ExitCode::from(4);
                }
            }
            match format {
                Format::Json => {
                    let mut value = serde_json::to_value(&report).expect("report serialization");
                    value["artifacts_dir"] = serde_json::json!(run.path);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).expect("report serialization")
                    );
                }
                Format::Text => print!(
                    "{}",
                    diagnostics::render_report(
                        &report,
                        &path.display().to_string(),
                        &source,
                        cli.color.enabled(false)
                    )
                ),
            }
            if *ui && report.status == checker::Status::Violated {
                let result = (|| -> std::result::Result<(), Error> {
                    let primary = report.witness().ok_or_else(|| {
                        Error::new(Span::default(), "violation has no saved witness")
                    })?;
                    let traces: Vec<_> = std::iter::once(primary)
                        .chain(
                            report
                                .claims
                                .iter()
                                .filter_map(|c| c.witness.as_ref())
                                .filter(|t| !std::ptr::eq(*t, primary)),
                        )
                        .collect();
                    if traces.len() > 128 {
                        return Err(Error::new(
                            Span::default(),
                            "explorer limit: more than 128 witnesses; replay an explicit trace",
                        ));
                    }
                    let mut bytes = 0;
                    for trace in &traces {
                        let size = serde_json::to_vec(trace)
                            .expect("trace serialization")
                            .len();
                        bytes += size;
                        if size > 16_000_000 || bytes > 64_000_000 {
                            return Err(Error::new(
                                Span::default(),
                                "explorer limit: evidence too large; use text replay",
                            ));
                        }
                    }
                    let mut session =
                        flareml::explorer::Session::validated(source.clone(), primary.clone(), &p)?;
                    for trace in traces.into_iter().skip(1) {
                        session.add_witness(flareml::explorer::Session::validated(
                            source.clone(),
                            trace.clone(),
                            &p,
                        )?);
                    }
                    explorer_server::serve(session, *no_open)
                        .map_err(|e| Error::new(Span::default(), e))
                })();
                if let Err(e) = result {
                    return fail("TOOL_ERROR", &e, &source);
                }
            }
            ExitCode::from(report.status.exit_code())
        }
        Command::Replay {
            model,
            trace,
            witness,
            ui,
            no_open,
            ..
        } => {
            let result = (|| -> std::result::Result<(Trace, flareml::model::Program), Error> {
                let data = read(&replay.as_ref().unwrap().1, 16_000_000)
                    .map_err(|e| Error::new(Span::default(), e))?;
                let artifact: Trace = serde_json::from_str(&data)
                    .map_err(|e| Error::new(Span::default(), format!("invalid trace JSON: {e}")))?;
                let p = flareml::compile(&source, Some(&artifact.check))?;
                if !ui {
                    artifact.validate(&source, &p)?;
                }
                Ok((artifact, p))
            })();
            match result {
                Ok((t, p)) => {
                    if *ui {
                        let mut session =
                            match flareml::explorer::Session::validated(source.clone(), t, &p) {
                                Ok(session) => session,
                                Err(e) => return fail("TOOL_ERROR", &e, &source),
                            };
                        if trace.is_none() {
                            let load = (|| -> std::result::Result<(), String> {
                                let report: serde_json::Value = serde_json::from_str(&read(
                                    &model.join("report.json"),
                                    16_000_000,
                                )?)
                                .map_err(|e| e.to_string())?;
                                let count = report["witness_files"]
                                    .as_array()
                                    .ok_or("missing witness list")?
                                    .len();
                                if count > 128 {
                                    return Err("explorer limit: more than 128 saved witnesses; use explicit model/trace paths".into());
                                }
                                let mut bytes = 0;
                                for index in 0..count {
                                    if index == witness.unwrap_or(0) {
                                        continue;
                                    }
                                    let (_, trace_path) = replay_paths(model, None, Some(index))?;
                                    let data = read(&trace_path, 16_000_000)?;
                                    bytes += data.len();
                                    if bytes > 64_000_000 {
                                        return Err("explorer limit: saved witnesses exceed 64 MB; use explicit model/trace paths".into());
                                    }
                                    let artifact: Trace =
                                        serde_json::from_str(&data).map_err(|e| e.to_string())?;
                                    let program = flareml::compile(&source, Some(&artifact.check))
                                        .map_err(|e| format!("{e:?}"))?;
                                    session.add_witness(
                                        flareml::explorer::Session::validated(
                                            source.clone(),
                                            artifact,
                                            &program,
                                        )
                                        .map_err(|e| format!("{e:?}"))?,
                                    );
                                }
                                Ok(())
                            })();
                            if let Err(message) = load {
                                return fail(
                                    "TOOL_ERROR",
                                    &Error::new(Span::default(), message),
                                    &source,
                                );
                            }
                        }
                        return match explorer_server::serve(session, *no_open) {
                            Ok(()) => ExitCode::SUCCESS,
                            Err(message) => {
                                fail("TOOL_ERROR", &Error::new(Span::default(), message), &source)
                            }
                        };
                    }
                    match format {
                        Format::Json => println!(
                            "{}",
                            serde_json::json!({"status":"REPLAY_VALIDATED","claim":t.claim,"steps":t.actions.len(),"loop_start":t.loop_start})
                        ),
                        Format::Text => {
                            println!(
                                "REPLAY_VALIDATED: {:?}\n{}",
                                t.claim,
                                diagnostics::render_trace(
                                    &t,
                                    &path.display().to_string(),
                                    &source,
                                    cli.color.enabled(false)
                                )
                            );
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    fail("TOOL_ERROR", &e, &source);
                    ExitCode::from(4)
                }
            }
        }
    }
}

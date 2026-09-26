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
        /// Parent directory for automatically saved, uniquely named run bundles.
        #[arg(long, default_value = ".fml/runs")]
        artifacts_dir: PathBuf,
    },
    /// Re-execute a current-format trace and validate its evidence.
    Replay {
        model: PathBuf,
        trace: PathBuf,
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
            ExitCode::from(report.status.exit_code())
        }
        Command::Replay { trace, .. } => {
            let result = (|| -> std::result::Result<Trace, Error> {
                let data = read(trace, 16_000_000).map_err(|e| Error::new(Span::default(), e))?;
                let artifact: Trace = serde_json::from_str(&data)
                    .map_err(|e| Error::new(Span::default(), format!("invalid trace JSON: {e}")))?;
                let p = flareml::compile(&source, Some(&artifact.check))?;
                artifact.validate(&source, &p)?;
                Ok(artifact)
            })();
            match result {
                Ok(t) => {
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

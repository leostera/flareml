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
    },
    /// Re-execute a current-format trace and validate its evidence.
    Replay {
        model: PathBuf,
        trace: PathBuf,
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
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
    let (path, format) = match &cli.command {
        Command::Check { model, format, .. } | Command::Replay { model, format, .. } => {
            (model, *format)
        }
    };
    let fail =
        |status: &str, e: &Error, source: &str| error(format, cli.color, status, e, path, source);
    let source = match read(path, 1_000_000) {
        Ok(s) => s,
        Err(e) => {
            fail("TOOL_ERROR", &Error::new(Span::default(), e), "");
            return ExitCode::from(4);
        }
    };
    match &cli.command {
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
                    fail("INVALID_MODEL", &e, &source);
                    return ExitCode::from(2);
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
                    let (status, code) = if internal {
                        ("TOOL_ERROR", 4)
                    } else if limit {
                        ("INCONCLUSIVE", 3)
                    } else {
                        ("INVALID_MODEL", 2)
                    };
                    fail(status, &e, &source);
                    return ExitCode::from(code);
                }
            };
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
                Format::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serialization")
                ),
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

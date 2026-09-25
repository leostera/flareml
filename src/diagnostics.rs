//! Terminal presentation is separate from machine-readable results and trace identity.
use crate::{
    checker::{Report, Status},
    semantics::Value,
    syntax::{ClaimKind, Error, Span},
    trace::Trace,
};
use owo_colors::OwoColorize;
use std::fmt::Write;
#[derive(Clone, Copy)]
enum Tone {
    Good,
    Bad,
    Warn,
    Accent,
    Dim,
}
fn paint(text: &str, tone: Tone, color: bool) -> String {
    if !color {
        return text.into();
    }
    match tone {
        Tone::Good => text.green().bold().to_string(),
        Tone::Bad => text.red().bold().to_string(),
        Tone::Warn => text.yellow().bold().to_string(),
        Tone::Accent => text.cyan().to_string(),
        Tone::Dim => text.dimmed().to_string(),
    }
}
fn safe(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_debug().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}
pub fn location(source: &str, span: Span) -> (usize, usize) {
    let mut end = span.start.min(source.len());
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = &source[..end];
    (
        prefix.bytes().filter(|b| *b == b'\n').count() + 1,
        prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
    )
}
pub fn render_trace(trace: &Trace, file: &str, source: &str, color: bool) -> String {
    let mut out = String::new();
    let title = if trace.kind == ClaimKind::Cover {
        "Reachability witness"
    } else {
        "Counterexample"
    };
    writeln!(
        out,
        "    {} · {} steps",
        paint(title, Tone::Accent, color),
        trace.actions.len()
    )
    .unwrap();
    for (name, rows) in &trace.states[0].tables {
        writeln!(
            out,
            "    {} {name} = {}",
            paint("initial", Tone::Dim, color),
            Value::List(rows.clone())
        )
        .unwrap();
    }
    for (i, action) in trace.actions.iter().enumerate() {
        if trace.loop_start == Some(i) {
            writeln!(
                out,
                "    {}",
                paint(&format!("╭─ LOOP START · state {i}"), Tone::Warn, color)
            )
            .unwrap();
        }
        let last = trace.kind != ClaimKind::Cover && i + 1 == trace.actions.len();
        writeln!(
            out,
            "    {} {}",
            paint(&format!("{:>2}.", i + 1), Tone::Dim, color),
            paint(
                &safe(&action.description),
                if last { Tone::Bad } else { Tone::Accent },
                color
            )
        )
        .unwrap();
        if action.id != "stutter" {
            let (line, col) = location(source, action.span);
            writeln!(
                out,
                "        {}",
                paint(&format!("{}:{line}:{col}", safe(file)), Tone::Dim, color)
            )
            .unwrap();
        }
        let before = &trace.states[i];
        let after = &trace.states[i + 1];
        for (table, rows) in &after.tables {
            if before.tables.get(table) != Some(rows) {
                writeln!(
                    out,
                    "        {}",
                    paint(
                        &format!("{table} := {}", Value::List(rows.clone())),
                        Tone::Warn,
                        color
                    )
                )
                .unwrap();
            }
        }
    }
    if let Some(i) = trace.loop_start {
        writeln!(
            out,
            "    {}",
            paint(
                &format!("╰─ repeat from state {i} forever"),
                Tone::Warn,
                color
            )
        )
        .unwrap();
        writeln!(
            out,
            "    {}",
            paint(
                if trace.weak_progress {
                    "Loop satisfies declared weak progress fairness."
                } else {
                    "No fairness assumption: a ready action may be postponed forever."
                },
                Tone::Dim,
                color
            )
        )
        .unwrap();
    }
    out
}
pub fn render_report(report: &Report, file: &str, source: &str, color: bool) -> String {
    let mut out = String::new();
    let (symbol, title, tone) = match report.status {
        Status::VerifiedInScope => ("✓", "VERIFIED IN SCOPE", Tone::Good),
        Status::Violated => ("✗", "VIOLATED", Tone::Bad),
        Status::Inconclusive => ("?", "INCONCLUSIVE", Tone::Warn),
    };
    writeln!(
        out,
        "\n  {}  {}",
        paint(&format!("{symbol} {title}"), tone, color),
        paint(&safe(&report.check), Tone::Accent, color)
    )
    .unwrap();
    writeln!(
        out,
        "  {}",
        paint(
            "────────────────────────────────────────────────────────",
            Tone::Dim,
            color
        )
    )
    .unwrap();
    for c in &report.claims {
        let (symbol, tone) = match c.result.as_str() {
            "VERIFIED_IN_SCOPE" | "REACHED" => ("✓", Tone::Good),
            "VIOLATED" => ("✗", Tone::Bad),
            "UNREACHABLE" => ("○", Tone::Warn),
            _ => ("?", Tone::Warn),
        };
        let kind = match c.kind {
            ClaimKind::Invariant => "invariant",
            ClaimKind::Property => "property",
            ClaimKind::Cover => "cover",
        };
        writeln!(
            out,
            "\n  {} {} {}",
            paint(symbol, tone, color),
            paint(kind, Tone::Dim, color),
            paint(&safe(&c.name), tone, color)
        )
        .unwrap();
        let (line, col) = location(source, c.span);
        writeln!(
            out,
            "    {}",
            paint(
                &format!("{}:{line}:{col} · {}", safe(file), c.result),
                Tone::Dim,
                color
            )
        )
        .unwrap();
        if let Some(note) = &c.note {
            writeln!(out, "    {}", paint(&safe(note), Tone::Warn, color)).unwrap();
        }
        if let Some(trace) = &c.witness {
            out.push_str(&render_trace(trace, file, source, color));
        }
    }
    writeln!(
        out,
        "\n  {} states · {} edges · graph {}",
        paint(&report.states.to_string(), Tone::Accent, color),
        paint(&report.edges.to_string(), Tone::Accent, color),
        if report.complete {
            "complete"
        } else {
            "incomplete"
        }
    )
    .unwrap();
    writeln!(
        out,
        "  {} input slots · {} · fairness {}",
        report.input_slots,
        report.semantics,
        if report.weak_progress {
            "weak runtime.progress"
        } else {
            "none"
        }
    )
    .unwrap();
    writeln!(
        out,
        "  {}",
        paint(
            &format!(
                "Budgets: {} states / depth {} / {} ms",
                report.max_states, report.max_depth, report.timeout_ms
            ),
            Tone::Dim,
            color
        )
    )
    .unwrap();
    if !report.not_checked.is_empty() {
        writeln!(
            out,
            "  Not selected: {}",
            report
                .not_checked
                .iter()
                .map(|n| safe(n))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    if let Some(cutoff) = &report.cutoff {
        writeln!(
            out,
            "\n  {}",
            paint(&format!("Cutoff: {}", safe(cutoff)), Tone::Warn, color)
        )
        .unwrap();
    }
    writeln!(
        out,
        "\n  {}",
        paint(
            "Model assumptions (not production guarantees)",
            Tone::Dim,
            color
        )
    )
    .unwrap();
    for assumption in &report.assumptions {
        writeln!(
            out,
            "    {}",
            paint(&format!("• {}", safe(assumption)), Tone::Dim, color)
        )
        .unwrap();
    }
    out
}
pub fn render_error(error: &Error, file: &str, source: &str, color: bool) -> String {
    let start = error.span.start.min(source.len());
    let len = error.span.end.min(source.len()).saturating_sub(start);
    let diagnostic = miette::MietteDiagnostic::new(safe(&error.message))
        .with_labels(vec![miette::LabeledSpan::at((start, len), "here")]);
    // Preserve byte offsets while preventing raw source from injecting terminal controls.
    let printable_source: String = source
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                "?".repeat(c.len_utf8())
            } else {
                c.to_string()
            }
        })
        .collect();
    let report = miette::Report::new(diagnostic)
        .with_source_code(miette::NamedSource::new(safe(file), printable_source));
    let theme = if color {
        miette::GraphicalTheme::unicode()
    } else {
        miette::GraphicalTheme::unicode_nocolor()
    };
    let mut out = String::new();
    miette::GraphicalReportHandler::new()
        .with_theme(theme)
        .render_report(&mut out, report.as_ref())
        .expect("String formatting");
    out
}

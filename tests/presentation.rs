use flareml::{
    checker::{self, Options},
    diagnostics,
};
use std::process::Command;
const BUG: &str = include_str!("../examples/missing-reply.fml");

#[test]
fn source_diagnostics_escape_terminal_controls() {
    let source = "property \"x\" { \u{1b}[31m }";
    let error = flareml::syntax::Error::new(
        flareml::syntax::Span { start: 16, end: 17 },
        "bad \u{1b}[31m token",
    );
    let rendered = diagnostics::render_error(&error, "model.fml", source, false);
    assert!(!rendered.contains('\u{1b}'));
}
#[test]
fn colored_and_plain_reports_share_content() {
    let p = flareml::compile(BUG, None).unwrap();
    let r = checker::check(BUG, &p, &Options::default()).unwrap();
    let colored = diagnostics::render_report(&r, "model.fml", BUG, true);
    let plain = diagnostics::render_report(&r, "model.fml", BUG, false);
    assert!(colored.contains("\x1b["));
    assert!(!plain.contains("\x1b["));
    for text in [
        "VIOLATED",
        "Counterexample",
        "process Inc",
        "Model assumptions",
    ] {
        assert!(colored.contains(text));
        assert!(plain.contains(text));
    }
}
#[test]
fn colors_are_controllable_and_never_leak_into_json() {
    for (flags, color) in [
        (vec!["--color", "always"], true),
        (vec!["--color", "never"], false),
        (vec![], false),
        (vec!["--color", "always", "--format", "json"], false),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_fml"))
            .args(["check", "examples/missing-reply.fml"])
            .args(&flags)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).contains("\x1b["),
            color
        );
        if flags.contains(&"json") {
            serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap();
        }
    }
}
#[test]
fn no_color_environment_is_respected() {
    let out = Command::new(env!("CARGO_BIN_EXE_fml"))
        .env("NO_COLOR", "1")
        .args(["check", "examples/counter-replies.fml"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("\x1b["));
}
#[test]
fn user_supplied_control_codes_are_escaped() {
    let src = BUG.replace(
        "a waiting client eventually receives its reply",
        "a waiting client \\u001b[31m",
    );
    let p = flareml::compile(&src, None).unwrap();
    let r = checker::check(&src, &p, &Options::default()).unwrap();
    let rendered = diagnostics::render_report(&r, "model.fml", &src, false);
    assert!(!rendered.contains('\x1b'));
}

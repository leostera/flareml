#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|source: &str| {
    if source.len() > 16_384 {
        return;
    }
    if let Ok(program) = flareml::compile(source, None) {
        let options = flareml::checker::Options {
            max_states: 32,
            max_depth: 8,
            timeout: std::time::Duration::from_millis(5),
            property: None,
        };
        if let Err(error) = flareml::checker::check(source, &program, &options) {
            assert!(
                !error.message.starts_with("internal:")
                    && !error.message.starts_with("invalid trace:"),
                "accepted source caused a checker failure: {error}"
            );
        }
    }
});

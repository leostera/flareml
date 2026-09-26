#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len() > 65_536 {
        return;
    }
    if let Ok(trace) = serde_json::from_slice::<flareml::trace::Trace>(data) {
        const SOURCE: &str = include_str!("../../examples/actor-messages.fml");
        static PROGRAM: std::sync::OnceLock<flareml::model::Program> = std::sync::OnceLock::new();
        let program =
            PROGRAM.get_or_init(|| flareml::compile(SOURCE, None).expect("checked fixture"));
        let _ = trace.validate(SOURCE, program);
    }
});

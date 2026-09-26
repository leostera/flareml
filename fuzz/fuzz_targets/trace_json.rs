#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len() > 65_536 {
        return;
    }
    if let Ok(trace) = serde_json::from_slice::<flareml::trace::Trace>(data) {
        static PROGRAMS: std::sync::OnceLock<Vec<(&str, String, flareml::model::Program)>> =
            std::sync::OnceLock::new();
        let programs = PROGRAMS.get_or_init(|| {
            [
                include_str!("../../examples/counter-replies.fml"),
                include_str!("../../examples/explicit-startup.fml"),
                include_str!("../../examples/spawn-workers.fml"),
                include_str!("../../examples/spawn-choice-workers.fml"),
                include_str!("../../examples/faulty-link-loss.fml"),
                include_str!("../../examples/faulty-link-duplicate-bug.fml"),
                include_str!("../../examples/faulty-link-duplicate-fixed.fml"),
            ]
            .into_iter()
            .map(|source| {
                (
                    source,
                    flareml::trace::source_hash(source),
                    flareml::compile(source, None).expect("checked fixture"),
                )
            })
            .collect()
        });
        if let Some((source, _, program)) = programs
            .iter()
            .find(|(_, hash, _)| *hash == trace.source_hash)
        {
            let _ = trace.validate(source, program);
        }
    }
});

use flareml::{
    checker::{self, Options},
    compile,
};

#[test]
fn actor_renaming_and_declaration_order_preserve_results() {
    for source in [
        include_str!("../examples/lost-update.fml"),
        include_str!("../examples/atomic-increments.fml"),
        include_str!("../examples/counter-replies.fml"),
    ] {
        let original = compile(source, None).unwrap();
        let expected = checker::check(source, &original, &Options::default()).unwrap();
        // Reverse lexical scheduling order as well as actor names.
        let renamed = source
            .replace("Client", "ZParticipant")
            .replace("Store", "AStorage")
            .replace("Counter", "ACount");
        let mut model = flareml::syntax::parse(&renamed).unwrap();
        model.actors.reverse();
        model.functions.reverse();
        model.types.reverse();
        let program = flareml::model::Program::build(model, None).unwrap();
        let actual = checker::check(&renamed, &program, &Options::default()).unwrap();
        assert_eq!(actual.status, expected.status);
        assert_eq!(actual.complete, expected.complete);
        for (a, b) in actual.claims.iter().zip(&expected.claims) {
            assert_eq!(a.result, b.result);
            if let Some(trace) = &a.witness {
                trace.validate(&renamed, &program).unwrap();
            }
        }
    }
}

#[test]
fn larger_bounds_preserve_concrete_counterexamples() {
    let source = include_str!("../examples/lost-update.fml");
    for bound in [2, 3, 4] {
        let source = source.replace("mailbox_bound = 2", &format!("mailbox_bound = {bound}"));
        let program = compile(&source, None).unwrap();
        let report = checker::check(&source, &program, &Options::default()).unwrap();
        assert_eq!(report.status, checker::Status::Violated);
        report
            .witness()
            .unwrap()
            .validate(&source, &program)
            .unwrap();
    }
}

# Modeling scenarios

All examples use the same current language and FIFO/atomic-turn execution contract. No semantics selector or legacy syntax exists.

| Example | Question | Expected result |
| --- | --- | --- |
| [sequential-workflow.fml](sequential-workflow.fml) | Does a single participant progress through observable preparation/commit steps? | Verified; intermediate states reachable |
| [eligibility-check.fml](eligibility-check.fml) | Can a pure policy deny an ineligible request? | Verified |
| [counter-replies.fml](counter-replies.fml) | Are replies processed after commit, and does generated work progress fairly? | Verified; reply reachable |
| [missing-reply.fml](missing-reply.fml) | Does returning state implicitly send a reply? | Violated: fair missing-reply lasso |
| [routed-deposits.fml](routed-deposits.fml) | Do transferable addresses route deposits to isolated keyed accounts? | Verified; both deposits reachable |
| [lost-update.fml](lost-update.fml) | Can two read-modify-write clients lose an update across protocol turns? | Violated: both clients finish but storage contains one increment |
| [atomic-increments.fml](atomic-increments.fml) | Does making increment a single storage turn repair the lost update? | Verified; both acknowledgments reachable |

```sh
cargo run --locked -- check examples/counter-replies.fml --trace-out /tmp/replies.json
cargo run --locked -- replay examples/counter-replies.fml /tmp/replies.json
cargo run --locked -- check examples/lost-update.fml --trace-out /tmp/lost.json # exit 1 by design
cargo run --locked -- replay examples/lost-update.fml /tmp/lost.json
```

Actors are a computational abstraction, not deployment categories. The storage examples are abstract protocols, **not D1 or database product adapters**. If the real system exposes intermediate effects, the model must split them into separate turns. A passing model does not prove implementation conformance.

Every `.fml` file here must be listed in `tests/examples.rs`, which checks its verdict and replays every generated witness. Regenerate artifacts when source changes.

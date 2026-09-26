# Requests for Discussion

RFDs record proposed and accepted design decisions for FlareML. A draft is not an implementation contract until reviewed and accepted.

| RFD | Status | Summary |
| --- | --- | --- |
| [0001 - Initial language and model checker](RFD0001-initial-language-and-model-checker.md) | Draft | Initial Cloudflare-first syntax, Rust CLI, invariant and temporal checking, finite scopes, and replayable counterexamples. |
| [0002 - Functions and actors as the modeling core](RFD0002-functions-and-actors.md) | Draft | Proposes state-in/state-out actors, typed asynchronous `send`, and one `property` declaration with explicit temporal/reachability operators; the unified claim surface is not yet implemented. |

Implementation tracking: [RFD0002 acceptance checklist](RFD0002-implementation-checklist.md) distinguishes tested generic-core work from the remaining adapter/release gates.

Use `RFD####-lowercase-hyphenated-title.md`, allocating the next unused number. Preserve existing numbers. Keep design rationale in the RFD; track implementation progress separately when needed.

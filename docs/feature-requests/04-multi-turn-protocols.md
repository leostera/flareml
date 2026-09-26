# Feature request: readable multi-turn protocols with explicit atomic boundaries

**Status:** draft; not filed. **Priority:** medium; validate ergonomics before adding syntax.

## Problem

One handler turn is already atomic; a multi-part operation must currently be split into explicit messages/handlers to allow interleavings. This is semantically appropriate, but larger request/reply protocols can accumulate phase variants, correlation fields, and self-sends that obscure the precise atomic boundary. Conversely, putting the whole operation in one handler can accidentally exclude a real interleaving.

## Minimal scenario

A coordinator asks two independent participants whether they are ready, then sends them separate commit messages. Another request may arrive between either reply and commit. Compare a safe protocol with an intentionally flawed one that treats an old reply as a current decision. An example should make the stale-reply trace easy to read.

## Requested behavior

Explore a reusable, typed protocol/state-machine pattern—or minimal syntax sugar—that makes each turn, outstanding reply, and correlation ID visible while reducing boilerplate. The lowering should be inspectable: if an operation has three interleavable phases, it must still create three or more separately scheduled turns. Atomic local updates and outgoing sends within a turn should retain existing semantics. Annotate the turn boundary clearly in traces.

## Acceptance criteria

- Two versions of the scenario produce the expected reachable safe/unsafe outcomes without hand-copying the same reply/phase wiring throughout the model.
- Witnesses show the other request intervening between named phases; replay uses the actual lowered transition semantics.
- Existing actor models behave identically, with no implied synchronous RPC, suspension, cross-actor transaction or implicit timeout.

## Non-goals / design questions

This is **not** a request to make distributed operations atomic or to hide request/reply ordering. First prototype it as an example or library pattern; only add syntax if it materially reduces code without weakening visibility or type safety.

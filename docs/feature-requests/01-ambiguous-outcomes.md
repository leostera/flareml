# Feature request: opt-in ambiguous outcomes at named interaction points

**Status:** draft; not filed. **Priority:** high for failure-sensitive protocols.

## Problem

Handler-local `choose` now permits a typed faulty-link actor to branch on forwarding and, in a separate turn after the recipient commits, on reply delivery. A generic engine fault is still absent. Modeling the ambiguity at many interaction points requires protocol-specific wrapper actors, correlation/state wiring, and explicit client retries; evaluate whether a reusable library is sufficient before adding more syntax.

## Minimal scenario

A client sends `Apply(operationId)` to a service. The service records that ID and replies. Explore separately: reply received; request never processed; operation committed but reply not observed. The client may resend the *same* ID after an ambiguous outcome. Check that the service applies it at most once. The trace must say whether the effect committed and which observation was lost, rather than simply labeling an RPC `failed`.

## Requested behavior

Explore an **opt-in reusable typed pattern or named fault-point API** for ambiguous acknowledgement and bounded retry, building on `choose`. Keep fault-free delivery the default. Make the ordering and visibility of the service commit, response enqueue, failure, and retry explicit. Document whether this is transport loss, timeout, or both; don't conflate a failed request with a lost reply. Allow checks to select fault budgets/sites and emit replayable witnesses identifying the chosen fault branch.

## Acceptance criteria

- A small model exposes both successful delivery and commit-without-observed-reply, and a reachable retry of the same operation ID.
- A deliberately non-idempotent service produces a safety counterexample; an idempotent version avoids it in a closed, bounded check.
- Replay validates the selected fault and all committed effects. Existing fault-free models retain their current semantics and verdicts.
- Budget exhaustion is reported **inconclusive**, not as a blocked/lost message or a safety proof.

## Non-goals / design questions

No implicit automatic retry policy, real network timing distribution, or vendor-specific transport guarantee. Decide whether the mechanism is language syntax or a standard model library only after testing whether the library version keeps traces and models readable. This is distinct from process restart and durable state.

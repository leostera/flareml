# Feature request: sound partial-order reduction for independent turns

**Status:** draft; not filed. **Priority:** medium, after baseline correctness/coverage.

## Problem

Exact exploration without partial-order or symmetry reduction repeats many permutations of independent actors' turns. A model with multiple partitions and independent clients can exceed state/time budgets before the interesting shared-key interleavings are checked. Raising a budget alone may not make the check useful.

## Minimal scenario

Several keyed actors each process a local message; one pair of actors also exchanges a message. Compare explored states and verdicts with and without reduction as the number of independent actors grows. An intentionally unsafe shared-key variant must still yield a counterexample.

## Requested behavior

An explicit opt-in POR mode with a stated independence relation covering per-address FIFO queues, outgoing messages, optional submissions, observations (`inputs`/`messages`), fairness, reachability, and temporal properties. Unsupported claim classes should fall back to unreduced search or report that reduction is unavailable—not quietly change their meaning. Expose raw/reduced exploration counts and the selected mode in results and trace metadata; replay must validate the witness against original semantics.

## Acceptance criteria

- Differential tests compare reduced and unreduced verdicts and reachable predicates over exhaustively enumerated small protocols, including a known unsafe example.
- The independent workload shows a measured reduction without changing its result; message-order-dependent workload retains its counterexample.
- Inconclusive remains inconclusive unless exploration is complete under a justified reduction. Replay validates a reduced-search witness against unreduced transitions.

## Non-goals / design questions

No performance claim before benchmark data, no assertion of independence merely because actors have different addresses, and no symmetry reduction folded into the same first implementation. Soundness design and independent validation are prerequisites, as noted in RFD0002.

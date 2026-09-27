# Judge review history (exploratory, not preregistered outcomes)

The 24 agent generations retain their original frozen EvalKit scores. No candidate
was repaired, resumed, substituted or selectively retried. Supplemental runs use
EvalKit with a **frozen-artifact agent that makes zero model calls**, copying the
saved project and compiling/running it unchanged. Every applicable candidate gets
the same review corpus. These are additional tests, not additional independent
agent samples. Do not present them as preregistered treatment-effect estimates.

## Version 1

Before manifest generation, private-control review found that two manifest case
labels executed the same workload (8 unique workload/seed pairs, 12 labeled runs).
The stale-blob-to-missing mutant also survived. A frozen supplemental corpus
replaces the duplicate group with four forced reader/publication/collection gaps:
read old head; hold blob fetch; publish newer generation; collect old blob; resume
fetch. The reference retries; the mutant incorrectly returns missing and fails.

After inspecting the Rust TLA+ implementation, a generic node-ID-permutation
review was added for **all** checkpoint candidates. It preserves the public
contract while making physical nodes 1 and 2 the stable primary. Two permutations
× twelve original scenarios = 24 checks. This exposed hard-coded holder selection.

Results: `judge-review.json`. Source snapshot: local campaign
`review-harness-v1/`. Original manifest and checkpoint scores remain unchanged.

## Version 2

Code review identified an uncovered case: the same primary restarts with several
committed leases, sees only an expired replay prefix, and proposes before seeing
the still-valid tail. Unlike promotion, this is a same-primary restart: the public
protocol supplies no caught-up marker, and append acceptance is fenced by current
node/epoch, not by knowing the entire prior log. A four-unit recovery quarantine
is one valid conservative solution under the stated maximum lease duration.

The new corpus tested partial primary replay and replay of durably completed,
still-unexpired work, across all four seeds. This also exposed a flaw in the
initial private reference worker, which was fixed (not any candidate). A
no-recovery-fence mutation now fails with overlapping checkpoints, and a mutation
that drops durable-completion hints fails with an invalid checkpoint restart.

The supplemental time rule permits completion exactly at the deadline: the
specified deadline has not arrived *first*. The original judge instead gave
expiry precedence in a tie. Both four-unit and two-unit reference leases pass the
supplemental tests. Original agent scores are not retroactively rewritten.

Results: `recovery-review.json`; snapshot `review-harness-v2/`.

## Version 3: broadened recovery and actual active promotion

Version 2 used a three-entry recovery prefix. Review of its passing candidates
showed that an unsafe reset-to-first-holder policy could coincidentally choose
the already-active holder and survive that prefix. Version 3 tests **both two and
three prior entries**, without tailoring node IDs to any candidate. Separate
mutants cover reset-cursor and replay-derived holder selection.

It also promotes an already-caught-up active replica without pre-aborting its
work. The candidate's promotion response must actually abort it. The original
helper pre-aborted even caught-up nodes, so did not test this obligation. A
missing-promotion-abort mutation is now rejected with `primary checkpointing`.
Durable-completion replay remains covered. Four variants × four seeds = 16 checks.

The reference and two-unit-lease control pass every extended case. Mutation tests
assert the **specific intended safety failure**, not merely any failing result.

Results: `recovery-review-v3.json`; snapshot `review-harness-v3/`. Version 2 results
remain available, including candidates that passed that narrower corpus.

## Interpretation

Original scores overestimated correctness. Judge strengthening was partly
post-hoc and the suite is an exploratory pilot, not a clean confirmatory study.
The concrete new counterexamples are valid implementation defects nevertheless.
Coverage is still bounded; passing all tests is not proof. Future comparisons
should freeze the strengthened corpus before agent generation.

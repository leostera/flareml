# Manual model review — distributed-001 (in progress)

This is a diagnostic review, not a replacement for the frozen workflow scores.
A checker violation is not automatically a discovered implementation/design bug.
Intentional cover witnesses, wrong properties, environment-model errors and actual
missing protocol guards are separated below. A passing code judge does not prove
that the model justified the code.

## Checkpoint / Rust / FlareML

Trial `548e1997-51cf-4520-9065-43a74192e5d4`.

- Saved history: 2 invalid models, 1 invariant violation, 1 incomplete run,
  3 completed in-scope runs.
- The violation was `promotion state excludes checkpoint`, encoded as **never
  entering `Primary` at all**. An ordinary promotion refuted it. This is a wrong
  property, not a discovered lease-coordination defect.
- The replacement safety formula is `state != Primary || state != Running`,
  tautological for two different enum constructors. The final model has one
  actor, 12 states, and a fixed FIFO message script. It cannot represent two
  independently checkpointing replicas, delayed log application or crash replay.
- `DESIGN.md` explicitly admits those omissions. Its honesty is useful, but the
  green workflow gate does not establish the distributed safety requirements.
- The independently judged implementation passes all 12 original cases. No
  counterexample-driven implementation repair is demonstrated by this model.

## Checkpoint / Rust / TLA+

Trial `61ae1a3a-cb9c-4213-a447-377d0d4264f2`.

- Saved history: 7 nonverified/error invocations, 2 successful safety checks,
  2 ordinary invariant violations, and 2 explicitly labeled cover witnesses
  (`NotYetDone`). The cover violations are **not bugs**.
- `Unexpired` first failed because time advanced without the model atomically
  enforcing the runtime's guaranteed deadline stop. The model was corrected to
  represent that environment guarantee. This is an environment-encoding repair.
- `AtMostOne` produced a concrete seven-state trace: the lease belongs to node 1,
  but nodes 1 and 2 both learn it and start. The `Start` action lacked
  `log[s].holder = n`; that eligibility guard was added before implementation.
  This is a real missing guard in the authored protocol model, not a parse error
  or an intentional witness. It does **not** show the baseline would have omitted
  the guard, nor demonstrate repair of an already-written implementation.
- The final model represents three logical replicas, separate issue/commit/apply
  and start steps, time, completion sets and promotion. Its reported exhaustive
  run has 577,793 distinct states. Bounds: two terms, two entries, time 0–8.
- Important omissions remain: no crash/recovery action or independently lagging
  local views; proposals inspect the global log. The code's four-tick recovery
  quarantine is justified outside the model. Quantitative stable-suffix progress
  is not proved; the cover only reaches one completed checkpoint.
- The implementation passes all 12 original cases, but the agent exceeded the
  token budget (624,390 reported total tokens). Completion failure and code
  correctness are separate outcomes.

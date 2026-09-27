# Assisted Go payment failure: minimal reproduction

Original run: `01ec878d-03e9-42ab-941d-0a4d469c3bfd`  
Original trial: `da92feab-8f38-4d36-a664-348ae8812fc9`

The original candidate **compiles**, but fails all 24 frozen payment schedules.
It was interrupted at the 600,000-token budget (615,280 reported tokens observed
at the response boundary), and no `DESIGN.md` was present. The candidate has not
been repaired or substituted in the evaluation results.

## Independent reproduction

Sent these five JSON lines to the original evaluator-built binary, outside the
scenario scheduler:

```jsonl
{"kind":"init","config":{"balance":10},"worker":0}
{"kind":"request","id":"r0","request":{"op":"charge","key":"a","amount":1}}
{"kind":"result","token":"t1","value":null}
{"kind":"result","token":"t2","ok":true,"value":{"balance":10,"payments":{}}}
{"kind":"result","token":"t3","value":{"balance":10,"payments":{}}}
```

Actual stdout:

```jsonl
[]
[{"key":"payment-ledger","kind":"get","token":"t1"}]
[{"expected":null,"key":"payment-ledger","kind":"cas","token":"t2","value":{"balance":10,"payments":{}}}]
[{"key":"payment-ledger","kind":"get","token":"t3"}]
[{"id":"r0","kind":"reply","result":{"status":"conflict"}}]
```

There is no existing application key with a different amount, so `conflict` is
invalid. After initializing the ledger, the worker should retain the request,
propose its debit, and acknowledge `charged` only after that CAS succeeds.

## Cause and modeling gap

In the successful CAS-completion branch, `main.go` calls
`delete(w.requests, p.id)` **before** checking whether `p.proposed == nil`.
Initialization uses a nil proposal; the worker then issues another get without
its original request. The next lookup yields a zero-value Go `Request`, whose
empty operation takes the default `conflict` branch.

The saved `design.fml` independently rechecks as `VERIFIED_IN_SCOPE` with 81
states. That is evidence about its finite model—not about request-context
lifetime in this Go implementation. The initialization path responsible for this
failure is not represented. The model also collapses both `A` and `B` onto the
same `a` payment field, never uses `b`, and chooses CAS success/failure without
checking the expected value. Refunds are absent. Thus its verified claim should
not be read as verification of the full payment protocol or a faithful CAS
implementation. This is a concrete example of why workflow/model verification
scores must remain separate from implementation correctness.

Its saved history contains seven `INVALID_MODEL` reports before the verified
one: missing statement terminators, reserved identifiers, and unknown `store` /
`self` names. The run demonstrates onboarding friction as well as a model-to-code
gap; it does not demonstrate discovery and repair of a service-level design bug.

The trial hit its token limit during unfinished development, so this does **not**
establish that an unconstrained assisted attempt would retain the bug. It is an
outcome under the common budget, not a claim that FlareML inherently causes this
particular defect.

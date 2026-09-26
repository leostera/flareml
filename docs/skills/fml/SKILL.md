---
name: flareml
description: Write and check finite systems models using the installed fml CLI. Use for .fml syntax, actor protocols, properties, bounds, traces, and replay.
---

# FlareML: agent overview

This guide and its detailed manual topics are **embedded in the installed `fml` binary**. No repository checkout, network access, or separate documentation install is needed. Use the installed binary as the authority for its version; `fml --version` prints that version. Read the relevant topic before authoring a model:

| Command | What it explains |
| --- | --- |
| `fml skills syntax` | Types, explicit statement semicolons, functions, `choose`, patterns, and precedence |
| `fml skills actors` | Actor definitions, explicit setup and spawn, messaging, scheduling, and assumptions |
| `fml skills properties` | Safety, reachability, temporal logic, fairness, and vacuity |
| `fml skills checks` | Finite pools, inputs, bounds, fairness, and exploration |
| `fml skills observations` | `instances(A)`, `inputs(A)`, and `messages(A)` stable views |
| `fml skills cli` | Commands, verdicts, exit codes, automatic run bundles, and replay |

`fml skills --help` lists topics; `fml check --help` and `fml replay --help` list runtime options. Each topic is standalone Markdown. You can save this overview as an agent skill with `fml skills > SKILL.md`; it instructs the agent to query topics on demand. To share the manuals without the binary, export each topic with `fml skills TOPIC > TOPIC.md`.

## First principles

FML is a finite systems modeling language with a native checker, **not a deployable runtime**. Use pure functions for local decisions and actors for participants with independent identity, state, mailbox, or scheduling. Actors are *not* Cloudflare Workers, Durable Objects, Queues, databases, or automatic durability. Model a real operation as multiple turns when something can interleave between its steps; a single FML callback otherwise commits its state and outgoing messages atomically. The generic engine assumes fault-free per-address FIFO delivery and optional external submissions; it does not inject loss, duplication, crashes, retries, timers, or persistence. `choose` can branch an actor turn over explicit modeled outcomes (such as forwarding or dropping a packet); no product guarantee follows. A passing check is not a production conformance proof.

An actor declaration defines a type, not an address or an eager population. Each `check` requires deterministic `main { ... }` setup and a `spawn_bound` per actor definition; it selects finite domains, optional inputs, and bounds. `main` runs once before exploration, not as an actor turn. A `property` explicitly asks `always P` (safety), `reachable P` (some finite execution), or a supported temporal claim such as `P leads_to Q` (all relevant executions, under declared fairness). `eventually P` is **not** the same as `reachable P`. External `once send(...)` is optional, even with weak runtime fairness. A bound or timeout makes the result inconclusive, not verified. The no-persistence assumption concerns the modeled application/system; separately, `fml check` persists evidence bundles, described in `fml skills cli`.

## Small runnable example

Save this as `workflow.fml`:

```fml
type Phase = Idle | Prepared | Finished
type Event = Step(Actor<Workflow>)

actor Workflow {
  init(): Phase { Idle }
  handle_message(state: Phase, event: Event): Phase {
    match state {
      | Idle -> {
          match event { | Step(me) -> { send(me, Step(me)); Prepared } }
        }
      | Prepared -> Finished
      | Finished -> Finished
    }
  }
}

property "created participants have state" {
  always (forall (w in instances(Workflow)) { w.created implies w.state != None })
}
property "preparation is possible" {
  reachable (exists (w in instances(Workflow)) { w.state == Some(Prepared) })
}
property "submitted work finishes" {
  forall (i in inputs(Workflow)) {
    i.submitted leads_to (exists (w in instances(Workflow)) { w.state == Some(Finished) })
  }
}

check WorkflowScenario {
  spawn_bound Workflow = 1
  mailbox_bound = 1
  main {
    let workflow = spawn(Workflow);
    inputs { once send(workflow, Step(workflow)) }
  }
  fairness { weak runtime.progress }
}
```

Run `fml check workflow.fml`. It saves a run bundle under `.fml/runs/<run-id>/` containing a source snapshot, configuration and report, plus every available witness; replay an indexed witness with `fml replay .fml/runs/<run-id>/model.fml .fml/runs/<run-id>/witnesses/0000.json`. Use `--artifacts-dir` to change the parent directory, `--trace-out witness.json` to export one witness separately, `--format json` for tooling, `--check WorkflowScenario` to select a check, or `--property "preparation is possible"` to select a claim. `reachable` reports `REACHED` or `UNREACHABLE` (the latter is not a failed requirement); verified/unreachable claims have no witness. See `fml skills cli` for reports, cutoffs, and exit codes.

## If the source tree is available

`README.md` has the quick start; `examples/README.md` indexes tested scenarios, including explicit startup, bounded worker creation, faulty links, and inventory/payment bug/repair pairs; `docs/rfds/RFD0002-functions-and-actors.md` states the core contract; `docs/rfds/RFD0003-nondeterministic-choice-and-faulty-links.md` specifies choice and faulty links; `docs/rfds/RFD0002-implementation-checklist.md` tracks remaining validation. Those files are optional and might describe a different checkout revision: prefer this installed binary's skills and checker when they disagree.

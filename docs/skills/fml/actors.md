---
title: FML actors and execution manual
description: "Actor definitions, deterministic setup, bounded spawning, typed references, atomic turns, and scheduling."
---

# FML actors and execution manual

This page is embedded in the installed `fml` binary. See `fml skills syntax` for data/functions, `fml skills checks` for setup and bounds, and `fml skills properties` for claims and observations.

## Actor definitions are not instances

An `actor` declaration defines a participant type; it creates **no instance**. Every selected check has a deterministic `main { ... }` setup block that constructs its initial population with `spawn`. A handler can create later instances with the same operation.

```fml
type Increment = Increment
actor Counter {
  init(): Int { 0 }
  handle_message(state: Int, message: Increment): Int { state + 1 }
}
property "increment is possible" {
  reachable (exists (counter in instances(Counter)) { counter.state == Some(1) })
}

check OneIncrement {
  domain Int = 0..1
  spawn_bound Counter = 1
  mailbox_bound = 1
  main {
    let counter = spawn(Counter);
    inputs { once send(counter, Increment) }
  }
  fairness { weak runtime.progress }
}
```

Each check requires one `spawn_bound Type = N` (`0..4096`) for **every** actor definition, including unused types. Setup and handlers share each type's lifetime pool. Identities are allocated monotonically from zero per type; they are never reused or deallocated. Exhaustion is inconclusive, not a rejected `spawn`, silent drop, or verification. A bound is a checker resource/scope, not an application admission policy.

`main` executes exactly once, before property observation and state exploration. Its bindings are local and do not become global names. `main {}` creates nothing. Setup may call deterministic helpers, spawn, send guaranteed initial messages, and register optional inputs. Setup cannot choose or inspect state/observations. Nothing processes a message until setup completes; failure or cutoff exposes no partial initial state.

- `let worker = spawn(Worker);` creates a fresh reference and instance.
- `spawn(Worker);` as a direct statement is also allowed; the instance remains, but gets no implicit startup message.
- `send(worker, Start);` in setup puts guaranteed work in the initial mailbox.
- `inputs { once send(worker, Start) }` captures an optional external slot; it is not queued until the checker later submits it.

There are no implicit singleton or keyed populations, eager identities, `.at(key)` addresses, `spawnable` modifier, or implicit self reference. Definition names are not routing references.

## State, initialization, and references

A stateful actor has `init(args...): State` and one `handle_message(state: State, message: Message): State`. `spawn(Type, args...)` supplies pure, typed arguments to `init`; initialization cannot send, choose, spawn, or inspect observations. A stateless actor omits `init`, takes only a message, and returns `unit`.

`Actor<A>` is a typed routing reference to one created instance of A, not a capability to inspect its state. References can be carried in data and messages. A creator can pass a reference to another actor or to the newly created actor itself. To self-send, first bind the result of `spawn`, then include that reference in an explicit message; there is no automatic self binding. General references do not provide a state lookup API.

Allocation effects may be used only as direct statements or whole local binding initializers in setup, handlers, and effectful helpers. An allocating helper follows the same placement rules. Allocation cannot be nested in a record, constructor, argument, property, initializer, domain, or input expression. Specification inspectors are likewise restricted to properties and pure specification helpers.

## Messages and atomic turns

`send(address, message)` is typed and one-way. It stages an outgoing message; returning a new state does not reply. Include a reply address/correlation ID in the protocol and send a separate response. Handlers cannot suspend or perform external I/O.

After setup publishes the initial state, the checker may stutter, submit an unsubmitted external input, or process the head of any nonempty mailbox. Input submission only enqueues; it does not run the target. A handler evaluates against its old state and commits the dequeue, next state, any new instances, and all staged sends atomically. Receivers run in later turns. Messages to one address are FIFO; different addresses can interleave.

A failed value check or capacity limit never partially commits a turn, allocation, observation, or outbox. The engine currently stops conservatively when a generated successor crosses a bound; it does not treat capacity as a blocked send or prune that branch while claiming verification. Atomic state-and-outbox publication is a model assumption, not a promise made by a database, network, or deployed service.

## Explicit faulty links and choice

A handler may bind `let outcome = choose([Deliver, Drop, Duplicate]);` to explore every compatible pure candidate. `choose` must be the entire initializer of a local binding in a handler or handler-only helper. It is not random, does not suspend a turn, and is not permitted in setup, initialization, properties, or inputs.

A link actor can explicitly forward, drop, or duplicate a packet while the engine's own mailboxes remain fault-free FIFO. A `Drop` still consumes the link's input turn, but queues no downstream message. Each complete choice branch commits as one atomic handler turn. Weak mailbox fairness schedules processing, not choice outcomes; a faulty link may choose `Drop` forever. See the faulty-link examples and RFD0003.

## Scheduling and fairness

The checker always permits stuttering. Without fairness, an enabled mailbox may be postponed forever. `fairness { weak runtime.progress }` rules out permanent postponement of a mailbox-processing action that remains continuously enabled. It does not force optional external input submissions, actor creation, or a favorable `choose` alternative. Safety and reachability inspect the full reachable graph; fairness does not prune reachable states.

## Inspecting the population

In specifications, `instances(A)` ranges over A's stable potential creation slots from the initial state, including instances that have not yet been created. Each slot exposes `created: Bool` and `reference: Option<Actor<A>>`; stateful actors additionally expose `state: Option<State>`. Before creation these are false/None; after creation the reference stays stable and the state reflects committed turns. Stateless instances have no `state` field. These views are specification-only. Quantify explicitly over them and account for unborn slots, for example:

```fml
property "created workers finish" {
  forall (worker in instances(Worker)) {
    worker.created leads_to worker.state == Some(true)
  }
}
```

A quantifier over zero creation slots can be vacuous. Pair universal claims with a reachability/creation claim when creation itself matters. Data-domain enumeration cannot manufacture actor references or enumerate potential unborn addresses; use `instances(A)`.

## Modeling boundary

The engine injects no loss, duplication, retries, timeouts, crashes, restarts, persistence, timers, or external I/O. Model selected outcomes explicitly if relevant. There is no termination, deallocation, identity reuse, supervision, dynamic input registration, suspension, reentrancy, or unbounded creation. Do not infer production conformance from an FML check. See `fml skills checks` for finite capacities and `fml skills properties` for result scope.

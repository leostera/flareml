# RFD0006 — Embedded interactive trace explorer

**Status:** proposed implementation contract; not implemented.

**Sequence:** implement this before [RFD0005 (suspension and reentrancy)](RFD0005-suspension-and-reentrancy.md). The explorer makes the current execution contract inspectable and provides a foundation for visualizing future segmented execution. It does not implement suspension or change model semantics.

## Motivation

A textual counterexample is useful evidence but becomes difficult to follow when several actors exchange messages, allocate workers, or make choices. We need to select any step and see the system at that point: participant state, FIFO queues, messages processed and sent, allocations, choices, and source locations.

Build a local browser application, distributed inside the FML binary. Contributors can use JavaScript tooling; people installing or running FML should not need it. Reuse React Flow for graph interaction, not as an execution engine or a substitute for accurate trace interpretation.

This is a **single-execution trace explorer**, not a visualization of the entire explored state graph, a production debugger, or an interactive scheduler.

## Decisions

- Root-level `explorer/` project using Bun, React, TypeScript, and Vite.
- React Flow (`@xyflow/react`) renders the selected system snapshot and transition highlights.
- A separate step list controls navigation. A swim-lane timeline is a later view, not forced into React Flow.
- `fml replay model.fml trace.json --ui` validates evidence, starts a loopback-only server, and opens the browser.
- Existing text/JSON replay stays available and remains the default for scripts and terminals.
- Compiled static assets ship in the crate and are embedded in the executable. No CDN, remote fonts, telemetry, or runtime JavaScript installation.
- Rust remains authoritative for compilation, replay, and evidence validation. The browser never reimplements FML evaluation, enabledness, fairness, or property checking.
- A transport-independent data-provider interface leaves room for a future WebAssembly backend. WASM and in-browser checking are not prerequisites.

## First user journey

```sh
fml check examples/spawn-workers.fml --trace-out /tmp/workers.json
fml replay examples/spawn-workers.fml /tmp/workers.json --ui

# Suitable for headless machines or manually selecting a browser:
fml replay examples/spawn-workers.fml /tmp/workers.json --ui --no-open
```

Replay can also use `model.fml` and a witness from an existing `.fml/runs/<id>/` bundle. The first milestone requires the explicit source/trace pair; automatic bundle discovery and selecting among all witnesses can follow.

1. Apply existing source/trace size and current-version checks, compile the selected check, and validate the complete trace.
2. If validation fails, report the normal replay error. Do not launch an apparently validated explorer.
3. Start a server on an OS-assigned port on `127.0.0.1`; print its URL and shutdown instructions.
4. Open that URL using the platform browser facility, without shell-interpolating paths or model text. If browser launch fails, keep the session available and explain how to open it manually.
5. The command stays alive until interrupted. Closing a tab does not terminate the session; Ctrl-C stops the listener and releases session data.

`--no-open` requires `--ui`. `--ui` conflicts with JSON output, so a long-running browser session never masquerades as a completed JSON replay command. Successful validation followed by normal user shutdown exits successfully; invalid evidence and server startup failures retain tool/replay error classification.

A future drag-and-drop loader must provide both source and trace to an authoritative validator. Parsing JSON or checking its shape alone must never produce a “validated” badge. Browser uploads and filesystem browsing are outside the first milestone.

## Interface

### Session header

Show the check and property names, obligation kind, source identity, trace/tool versions, declared fairness, and relevant bounds. Identify a reached witness versus a finite or looping counterexample.

Label the evidence precisely: “validated reached witness” or “validated counterexample,” not “the model is verified.” A trace alone does not contain the complete run report, exploration cutoff, or all property outcomes. If reports are supported later, display their results/completeness separately and match their source/check identity before associating them.

A cutoff is not itself a property violation. A verified run may have no witness at all; this feature does not invent one or pretend a trace explains a cutoff for which no path was saved.

### Step navigation

- Initial state, previous/next, first/last, and a selectable step list with keyboard equivalents.
- Steps labeled as external submission, actor processing, or stutter, with allocation/choice badges where applicable.
- Search/filter by actor or event kind; filtering never renumbers or deletes underlying transitions.
- A selected snapshot index is the single source of truth across panels.
- Source selection, actor selection, and message selection remain stable while navigating where their referents still exist.

For N actions, there are N+1 snapshots. Snapshot 0 is completed deterministic setup. Action i takes snapshot i to i+1; selecting its row shows snapshot i+1 and the diff from i. The UI must make this before/after convention visible.

Zero-action witnesses still have a useful initial-state view. A stutter is a real trace transition with an empty state diff, not a missing event.

### System graph

- One node per **created actor instance**, keyed by canonical typed identity rather than a display label.
- Show actor type, instance label, stateful/stateless distinction, pending mailbox count, and selected-step activity.
- Newly created actors appear at the committed snapshot in which they are published. Unborn potential slots are not live nodes; show pool usage and optional slot inspection separately.
- Highlight sends from the selected transition, with visible direction, payload summary, and multiplicity. Distinct equal-payload sends must remain individually inspectable.
- Show optional external submissions from an explicit environment marker. Initial setup sends have a setup marker, not an invented sender actor.
- Include self-sends and allocations without implying extra scheduling boundaries.

Edges represent observed events, not permanent network topology. References retained in state do not automatically imply a communication link. A later reference graph or aggregate traffic view must be separately labeled.

Use a simple deterministic initial layout and stable node positions, with pan, zoom, fit, and user dragging. Do not relayout every step. Keep future/unborn nodes hidden until creation even if their positions are prepared in advance.

### Inspector and source

Clicking an actor shows its exact current state and ordered mailbox; clicking an envelope shows payload, provenance, external-slot association, and lifetime observation identity when present. Display stateless actors as stateless, not as a user-visible unit state field.

Show structural before/after differences for state, queues, input flags, and message observations. Distinguish “not created,” “empty mailbox,” `None`, and absent optional message history.

Clicking an action exposes:

- the actor and dequeued message, or the external slot being submitted;
- committed state changes, sends, and fresh allocations;
- choice encounter order, candidates selected, values, and helper call sites;
- source spans for the handler, sends, choices, and allocations when available.

Source offsets are UTF-8 byte offsets. Supply correct source ranges/line information from Rust; do not treat them as JavaScript UTF-16 string offsets. Missing provenance is labeled unavailable, never guessed from a formatted description.

## Semantic fidelity

The [current contract](RFD0002-functions-and-actors.md) remains authoritative:

- A handler commits state, outbox, allocations, and completion together. Event ordering within a turn may be listed, but is not navigable as intermediate scheduler states.
- FIFO queue order is explicit. Processing removes the old head before appending self-sends.
- Input processing means that invocation committed, not that a reply or follow-up finished.
- Setup has completed before snapshot 0. No fabricated setup scheduling steps or implicit startup events.
- A lifetime message observation slot is not a reusable mailbox position.
- A choice selection records an outcome, not its probability or a fairness guarantee.

For a lasso, `loop_start` identifies a snapshot already revisited by the final snapshot. Mark both the loop entry and closing transition. The finite prefix plus repeatable loop is evidence for an infinite execution; it is not a measured duration, a wall-clock timeline, or a claim that the model terminated at the last row.

Mailbox/history bounds and fairness are assumptions, not sliders that can alter a validated trace. New experiments must produce new evidence through the checker.

## Validated presentation data

Do not infer events by parsing `Action.description`. It is presentation text, not a machine-readable event contract. Likewise, payload equality cannot establish message identity.

The current format-8 trace stores exact snapshots, choice/allocation records, and envelope provenance, but no complete structured outbox on each action. Add a **replay presentation projection** produced through the existing interpreter/commit path:

- Capture the consumed envelope, source-ordered sends, allocations, choice selections, and setup publication during constrained replay.
- Release a session as validated only after the existing state/action comparisons, property validation, and loop/fairness checks succeed.
- Reuse execution and validation code; do not create an explorer-specific interpreter or an unconstrained second execution of choice.
- Keep presentation recording optional so ordinary checking/replay does not pay unnecessary memory costs.

This projection is derived, disposable data, not another authoritative trace format. Prefer retaining format 8 unless implementation discovers genuinely missing persisted evidence. If persisted meaning/layout must change, bump the trace format explicitly; never add a compatibility execution mode to make old visualizations work.

### Viewer schema

Use a versioned presentation DTO, separate from the trace format version, with:

- session/source/check/claim metadata and actor definitions;
- canonical actor, external-slot, and trace-local envelope IDs;
- snapshot and transition summaries;
- detailed selected-state values, queues, observations, and transition events;
- source locations and loop entry information.

Rust `Int` is signed 64-bit. JavaScript JSON numbers cannot represent all those values exactly. Encode modeled integers as tagged decimal strings in the presentation schema. Never round values, compare them as floating-point numbers, or use lossy numeric values to construct identities. Actor/node IDs should be opaque strings supplied by Rust.

When lifetime message history is absent, assign **trace-local display IDs** during replay: initial setup enqueue order and subsequent enqueue events create distinct envelopes; FIFO dequeue tracks their identity. These IDs do not become model state or temporal observation slots and must not affect state equality. Even a stutter-like data cycle can have different display envelope IDs across one recorded loop traversal; loop equality remains the Rust model-state equality, not presentation-ID equality.

Check in DTO fixtures generated from real validated models. Establish one schema authority and an automated Rust/TypeScript conformance check rather than maintaining two silently diverging type definitions. Frontend shape validation catches integration errors; it is not semantic validation.

## Project and distribution

Proposed layout:

```text
explorer/
  package.json
  bun.lock
  index.html
  src/
    trace/          # DTO decoding, indexing, structural diffs, selection
    providers/      # Native HTTP adapter; future WASM adapter
    components/
    views/
  tests/
  dist/             # Committed, generated release assets
  README.md
```

Pin the Bun version and commit its lockfile. Keep parsing/indexing/presentation logic independent of React and React Flow. Use React Flow for interactive graph rendering; do not depend on its internal node state as the authoritative trace selection or actor identity store.

Commit the generated `dist/` assets so source installs, crates.io builds, and offline Cargo builds can embed them without running Bun. Include them explicitly in Cargo packaging. Rust builds must fail clearly if required assets are absent; no placeholder UI, network download, or JavaScript build in `build.rs`.

A frontend CI job installs locked dependencies, type-checks, tests, and rebuilds assets with the pinned toolchain. It checks that generated assets match the committed output. Builds must avoid timestamps or absolute-machine paths in output. Normal Rust CI remains stable-only and must work without Bun installed. Release validation checks the packaged crate, not just the checkout.

Bundle required third-party license notices alongside release assets. Check bundle size and review dependencies rather than importing a large dashboard framework for a small inspector.

## Local server and privacy

A local browser app still has a security boundary: hostile web pages may attempt requests to localhost, and model values are untrusted strings.

- Bind only to `127.0.0.1`, on an ephemeral port. No public bind flag in the first version.
- Serve only embedded assets and session-specific APIs, never a project directory or arbitrary filesystem paths.
- Require an unguessable per-session bearer token for source/trace APIs. Pass it to the app in the URL fragment, then use an authorization header; do not place secrets in query strings or access logs.
- Validate Host and, when present, Origin against the actual loopback origin. No permissive CORS; reject cross-origin API access and unexpected methods.
- Use a restrictive content-security policy and local resources only. Render model/source values as text, not HTML; no `eval` or executable content from traces. Test any style-policy allowances React Flow requires.
- Apply request/response limits, strict route allowlists, and no-cache headers for model/session data. Do not persist tokens or source in browser storage by default.
- Preserve existing source/trace limits. Bound presentation expansion, cache size, and per-request work as well; a compact trace must not cause unbounded browser or server allocation.

The browser receives only the selected session's validated model/evidence. No commands, model edits, check launch endpoints, remote uploads, or telemetry are introduced. Session URLs are capabilities and should not be shared casually. This protects against unintended web access, not a hostile process already controlling the user's machine.

## Responsiveness and limits

Existing traces may contain up to 100,000 actions, subject to the trace-file size limit. Do not send every full snapshot as a second enormous JSON document or mount one DOM row per event.

Provide session metadata and lightweight indexed summaries, paginated transition summaries, and on-demand snapshot/transition detail. Cache a bounded working set. Virtualize large step lists and value/queue inspectors. Filtering and graph layout must have visible, bounded work rather than freezing the page.

Large active populations may require a graph rendering threshold. If some actors or event edges are hidden, show counts and an explicit filter/limit notice; never make a partial drawing look like the complete system. All actors remain accessible through a searchable list. A UI rendering limit is not a checker cutoff and cannot change evidence validity.

## Future WebAssembly adapter

The Rust library is already separate from the CLI, making WASM a plausible next backend—not a proven drop-in build target.

Define a narrow async frontend provider interface for session metadata, summaries, and selected-state details. Its first implementation calls the local native server. A later implementation can compile and validate uploaded source/trace pairs using the **same Rust core** in a Web Worker and return the same presentation DTOs.

Before shipping WASM, audit target/dependency support, clocks and cooperative deadlines, memory limits, source hashing, and termination/cancellation. Do not rely on cancelling a JavaScript promise to stop synchronous Rust work. Test native/WASM agreement on validation results and projections. Offline standalone validation and browser-launched checks are separate milestones, with explicit budgets and version rules.

No JavaScript implementation of FML semantics, mandatory WASM artifact, or browser-side check launcher is part of this RFD's first deliverable.

## Implementation milestones and acceptance

### 1. Replay projection and fixtures

- [ ] Specify the viewer schema, IDs, source ranges, lossless values, and selection convention.
- [ ] Add optional structured recording through authoritative replay/setup execution.
- [ ] Test zero-step witnesses, stutters, setup sends, self-sends, equal payloads, multiple sends, choices, allocations, and fair loops.
- [ ] Verify every projected commit against the validated snapshots; test absent message history and stateless actors.
- [ ] Corrupted traces fail before a validated session is exposed.

### 2. Frontend explorer

- [ ] Create `explorer/` with locked Bun/React/TypeScript/Vite dependencies and React Flow.
- [ ] Implement step navigation, actor graph, queue/state inspector, structural diffs, event detail, and source selection.
- [ ] Mark setup, atomic transitions, witness type, and lasso boundaries accurately.
- [ ] Unit-test pure indexing/diff/selection logic, including large integers and Unicode source locations.
- [ ] Browser-test keyboard navigation, graph selection, repeated messages, no-history traces, and dynamic creation with real fixtures.

### 3. Embedded native delivery

- [ ] Add `replay --ui` and `--no-open`, preserving text/JSON replay and exit policies.
- [ ] Implement loopback session transport, authentication/origin protections, strict assets/routes, and bounded detail APIs.
- [ ] Embed committed release assets; verify clean `cargo package`/installation without Bun or Node.
- [ ] Exercise browser-launch failure, headless startup, shutdown, malformed requests, and source/trace errors.
- [ ] Add CI asset freshness checks, frontend tests, and third-party notices.

### 4. Evidence and usability

- [ ] Walk through explicit startup, lost update, missing reply, duplicate delivery, and choice/spawn examples.
- [ ] Confirm identical payloads never collapse identity and visual subevents never imply partial commits.
- [ ] Test hostile labels/source text, cross-origin requests, path traversal, and session isolation.
- [ ] Measure binary/bundle size, startup latency, memory, and navigation on larger traces; document rendering limits honestly.
- [ ] Update README, CLI manuals, and bundled skills with supported workflows and limitations.

Ship these together as a usable native explorer. Static diagram/HTML export, swim lanes, autoplay, bundle browsing, hosted deployment, WASM validation, check launching, trace editing, and the full explored-state graph remain follow-ups.

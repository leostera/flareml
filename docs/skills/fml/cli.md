---
title: FML CLI, results, and replay manual
description: "CLI commands, verdicts, exit codes, trace artifacts, and deterministic replay."
---

# FML CLI, results, and replay manual

This page is embedded in your installed `fml` binary. `fml skills` is the topic index; `fml skills syntax|actors|properties|checks|observations` provides the modeling manuals. For the exact installed option set use `fml --help`, `fml check --help`, `fml replay --help`, and `fml skills --help`.

## Commands

```sh
fml check model.fml
fml check model.fml --check Scenario --property "claim name"
fml check model.fml --max-states 100000 --max-depth 1000 --timeout 30s
fml check model.fml --format json --artifacts-dir /tmp/fml-runs
fml check model.fml --trace-out witness.json  # optional extra export of one witness
fml replay .fml/runs/<run-id> --format json
fml replay .fml/runs/<run-id> --ui
fml replay .fml/runs/<run-id> --witness 1 --ui --no-open
fml --version
fml skills actors
fml skills --install
```

`--check` chooses a named `check` experiment when there are multiple; `--property` checks only one named property and reports the others as not checked. Every `check` that successfully reads its source attempts to create a **run bundle** under `.fml/runs/` by default; `--artifacts-dir` changes its parent. `--trace-out` additionally exports *one* available counterexample or reached witness, not the whole bundle. A verified or unreachable claim has no witness, but the run still has a report. `--format json` writes a structured report to stdout and includes `artifacts_dir` when checking returns a report; the bundle path is also printed on stderr. Text reports support `--color auto|always|never` and `NO_COLOR`; JSON has no presentation ANSI. Use source-mapped errors and evidence, not only the exit code.

| Exit code | Meaning |
| --- | --- |
| 0 | Selected requirements verified in finite scope; reachability queries are reported separately as `REACHED` or `UNREACHABLE` |
| 1 | A requirement was violated, with finite or lasso evidence |
| 2 | Invalid or unsupported model/configuration |
| 3 | Inconclusive: exploration, domain, evaluation, or capacity limit |
| 4 | Tool/I/O or replay validation error |

A `reachable P` query returning `UNREACHABLE` is informational and not a failing exit. Only a **complete** explored graph establishes absence. A reached witness is valid even if further exploration hits a bound. A whole `always P` can fail as soon as an initial/successor bad state is found, before graph closure; most temporal properties require a complete finite graph for a definitive result. A timeout or cutoff is never proof. State-space search uses exact state equality without symbolic, symmetry, or partial-order reductions. Default budgets are 100000 states, depth 1000, and 30s (cooperative timeout).

## Run bundles, evidence, and replay

`fml replay <run-directory>` loads its bundled `model.fml` and first saved witness from `report.json`. `--witness N` selects a zero-based entry in the saved witness list, not a filename number. Runs with no witnesses cannot be replayed. Explicit `fml replay <model.fml> <trace.json>` remains supported. `fml check model.fml --ui` opens the explorer after saving artifacts if a property violation is found; passing checks do not launch it. The violation exit code remains 1 after the server closes. Add `--ui` to replay to open the embedded offline explorer, or `--ui --no-open` to print its private session URL. Bundle UI replay loads all saved witnesses into a shared-prefix tree; `--witness N` chooses the initial execution. The tree represents saved evidence, not all explored paths. Keep the complete URL, including its `#` token. Ctrl-C stops the local server.

Each completed run bundle contains `model.fml` (the exact source snapshot), `configuration.json` (selected model/check/property, state/depth/timeout limits, tool version, and source SHA-256), and `report.json` (status, completeness, cutoff, and an index of all available witness files). Each available witness is stored under `witnesses/0000.json`, `witnesses/0001.json`, etc.; names are numeric, never taken from property names. `report.json` is written last as a completion marker; its absence means bundle creation did not finish, **not** that a model is verified. Invalid models and initialization errors also produce error reports after a successful source read. Unreadable source or unwritable artifact storage returns a tool error (exit 4) without claiming verification. Bundles are records of checker output, not independently checkable proof certificates or a power-loss durability guarantee.

Violation traces contain a finite prefix for safety/until errors or a stem plus repeating loop for an infinite temporal counterexample. Reachability traces contain a finite path. Replay using the **saved `model.fml`**, not a later-edited original: it reruns deterministic `check.main`, compares the entire initial population, mailboxes, captured inputs and observations, then re-executes transitions and checks state snapshots, allocation freshness/initialization/bounds, provenance, source/check identity, loop closure, fairness, and the property evidence. There is no witness file for a verified or unreachable claim; an incomplete result must not fabricate evidence, though a reached witness discovered before cutoff may still be valid. The current trace format is **8**: every action has a choice transcript (empty for deterministic turns) recording encounter order, call-site stack, candidate position, and value, plus allocation records (empty when no actors are created). Replay constrains execution to that transcript and rejects missing, extra, reordered, or altered selections. Only current-format artifacts are accepted; regenerate traces after source or format changes. Do not edit witness JSON to make a result pass. Bundles contain modeled values/addresses; avoid real secrets and manage retention and access.

The CLI limits model source input to 1 MB, trace input to 16 MB, and replay to 100000 actions. CLI/tool errors are distinct from invalid model errors. `fml check` is not a code generator or deployment tool: a verified finite model remains conditional on its scope, its fault-free mailbox/atomic-turn assumptions, and the real system's conformance.

## Bundled agent installation

`fml skills --install` writes `SKILL.md` plus six topic `.md` files to `~/.agents/skills/flareml/` (using the process's home directory). Running it again is safe if files are unchanged. It refuses to overwrite a differing existing file or a symlink; back up/remove an old installation before updating to another binary version. `fml skills` and every topic also work without installation or the source checkout. For another agent skill directory, export the overview and topics with shell redirection rather than assuming a platform-specific discovery path.

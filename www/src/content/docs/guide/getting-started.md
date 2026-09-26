---
title: Getting started
description: Install the FlareML CLI, check a model, and replay its evidence.
---

FlareML lets you sketch how parts of a program interact and check those interactions before you write the implementation. It runs locally with no JVM, cloud account, or external service required.

## Install

```sh
cargo install flareml
fml --version
```

To build from a checkout instead, install the local package with its lockfile:

```sh
git clone https://github.com/leostera/flareml.git
cd flareml
cargo install --path . --locked
```

## Check and replay

Save the [link shortener model](./link-shortener/) as `link-shortener.fml`, then run:

```sh
fml check link-shortener.fml --trace-out /tmp/link-shortener.json
fml replay link-shortener.fml /tmp/link-shortener.json
```

Each check also saves its exact source, report, and all available witnesses in `.fml/runs/<run-id>/`; `--trace-out` simply exports one witness to a convenient path. `fml check --help` and `fml replay --help` list the available options. The [CLI manual](/reference/cli/) explains verdicts, exit codes, run bundles, traces, and replay.

## Keep learning

- [Your first system design](./link-shortener/) walks through a small example, from the first question to the messages and properties.
- [Language reference](./language/) links to the full syntax and semantics manuals.
- [Execution model](./execution-model/) explains turns, fairness, observations, traces, and replay.
- `fml skills` prints the same language and execution manuals from your installed CLI.

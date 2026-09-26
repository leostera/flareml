---
title: FML syntax manual
description: "The complete FML syntax reference: declarations, types, functions, expressions, quantifiers, and common mistakes."
---

# FML syntax manual

This page is embedded in your installed `fml` binary. Related: `fml skills actors`, `fml skills properties`, `fml skills checks`. Use `fml check model.fml` to type-check and explore a complete file; unsupported syntax is rejected, not ignored.

## File structure and types

Top-level declarations are `type`, `let` (a function), `actor` (a definition, not an instance), `property`, and `check`. No imports, namespaces, top-level constants, legacy `invariant`/`cover`, or `semantics` selector exist. `//` starts a line comment. Example:

```fml
type AccountId = Alice | Bob
type Request = Deposit(Int) | Query(Actor<Client>)
type Route = Route { recipient: Actor<Account>, amount: Int }
type Phase = Idle | Done
let increment = (n: Int): Int { n + 1 }
```

Types are closed variants, record variants, or transparent named aliases; variant payloads can be positional (`Deposit(1)`) or record fields (`Route { recipient: Account.at(Alice), amount: 1 }`). Built-ins: `Bool` (`true`, `false`), `Int`, `String`, `unit` (`()`), `Option<T>` (`Some(x)`, `None`), `Result<T, E>` (`Ok(x)`, `Err(e)`), and `Actor<A>` (typed address to an explicitly spawned instance of A). Strings use quotes. Data cannot use recursive types or user-defined generic types. Integer and string data require explicit finite literal pools in a `check`: e.g. `domain Int = 0..2`, `domain String = ["a", "b"]`. The checker validates stored values, messages, arguments, and return values against those pools; out-of-pool evaluation is inconclusive, not overflow/wrapping or silent pruning.

## Functions, blocks, and patterns

```fml
type Request = Eligible | Ineligible
type Decision = Allowed | Denied
let decide = (request: Request): Decision {
  match request { | Eligible -> Allowed | Ineligible -> Denied }
}
```

`let name = (param: Type, ...): ReturnType { statements }` declares a function. Return type omission means `unit`. Local `let x = expression` binds a value inside a function/handler; the final *unterminated* expression or exhaustive tail `match` returns its value. Every local binding and non-tail statement, including a non-tail `match`, **requires `;`**; whitespace/newlines are not separators. A final expression followed by `;` is discarded and the block returns `unit`. Match arms use `| Pattern -> expression` or `| Pattern -> { statements }`, not semicolon separators; `_` is a wildcard and bare names bind values (use declared constructors to match variants). Branch-local bindings do not escape. Patterns support variant payload bindings but not nested constructor matching, record destructuring, or ignored `Result` errors. Handle `Ok` and `Err` explicitly when binding a `Result`. Non-exhaustive matches, recursive local calls, and shadowing existing names are rejected.

Pure helpers can compute local results; setup and handlers may also use helpers with `send` or `spawn` effects, and handlers may use `choose` helpers, subject to transitive effect rules. An effectful helper must be used as a direct statement or binding initializer, never hidden inside a constructor, argument, record, or property. No host APIs or arbitrary Rust execution is available. Properties are **expressions**, not statement blocks; a property cannot contain a local `let` directly. Put local computation in a pure/specification helper called by the property. State or observation inspectors can be used only in specifications, not in handlers or initializers, even through helper calls.

## Handler-local nondeterministic choice

`let picked = choose([Deliver, Drop]);` branches over **all** candidates. The literal list must be nonempty and have compatible pure expressions; duplicate values still have distinct source positions in replay evidence. `choose` is the entire right-hand side of a local binding inside a handler or handler-only helper. It is not valid in deterministic setup, pure expressions, properties, init, input declarations, or nested arguments. It does not suspend the turn or choose at random. See `fml skills actors` for the atomicity/fairness rules. General computed collections are not enabled as executable handler data by this special literal-list syntax.

## Setup and allocation

An `actor A { ... }` is a definition only. Every `check C` requires `main { ... }` and a literal `spawn_bound A = N` for **each** actor definition, including those with zero instances. In setup or a handler, `let a = spawn(A, args...);` or a discarded `spawn(A, args...);` allocates a fresh `Actor<A>` address. Initializer arguments are pure; stateless actors accept none. Spawn must be a whole binding initializer or direct statement. `main` may register `inputs { once send(a, message) }` (not enqueued until optional submission) or call `send(a, message);` (guaranteed initially enqueued). An intermediate `inputs` block requires `;`. Setup cannot choose or inspect specifications. `instances(A)` in properties exposes stable future creation slots; there is no `A` singleton address or `A.at(key)`.

## Expressions and quantifiers

Expressions include literals, names, variant construction, records, field access (`value.field`), calls (`decide(request)`), lists (`[a, b]`), Boolean `!`, `&&`, `||`, `implies`, arithmetic `+`/`-`, comparisons `==`, `!=`, `<`, `<=`, `>`, `>=`, and property operators `always`, `eventually`, `reachable`, `leads_to`, `until`. `forall (x in Domain) { predicate }` and `exists (x in Domain) { predicate }` quantify **finite data**, including declared type domains or read-only `instances(A)`, `inputs(A)`, and `messages(A)` views. The latter are specification-only. `exists` is not existential quantification over execution paths; use top-level `reachable` for that.

Precedence, low to high: `leads_to`/`until`; `implies`; `||`; `&&`; `==`/`!=`; ordered comparisons; `+`/`-`; prefix operators; field and call postfix operations. Prefix `always`, `eventually`, and `reachable` bind tightly: write `always (A.state == Done)` rather than `always A.state == Done`. Parenthesize mixed Boolean/temporal expressions to make the intended claim explicit. The supported temporal fragment is deliberately restricted; read `fml skills properties` before relying on temporal nesting.

## Common mistakes

- `let x = 1` at file scope is **not** a constant declaration: top-level `let` takes typed function parameters and a body.
- `property "p" { A.state == Done }` needs an explicit `always`, `eventually`, or top-level `reachable`.
- `Actor<A>` is a routable address, not permission to inspect A's state from a handler. `A` itself names a definition, not an address.
- A model requires a `check` with `mailbox_bound = N`, `main { ... }`, and `spawn_bound A = N` for each definition. `check` configures the experiment; it is not invoked as a program entry point.
- Names of keywords cannot be reused as declarations. Keep models small and ask the checker for source-mapped errors rather than guessing syntax.

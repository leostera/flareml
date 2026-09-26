---
title: Your first system design
description: Sketch a tiny link-shortening service, describe what its parts should do, and explore whether the design behaves as intended.
---

Before building a web service, let's sketch how one should work. A client asks for a short link, gets a code back, and follows that code to the saved destination. It's a small story with a few parts—and the parts have to send the right messages in the right order. That makes it a useful first design to explore before writing the service itself.

We'll use two destinations and one client. We won't build an HTTP server or database, and we won't try to model every feature of a real shortener. First we'll describe the behavior we care about, then ask whether that behavior is possible and whether replies stay consistent.

The complete working example is [`examples/link-shortener.fml`](https://github.com/leostera/flareml/blob/main/examples/link-shortener.fml). The snippets below introduce it one idea at a time.

## 1. Start with the story

Here is the round trip we want:

1. The client asks to shorten `Docs`.
2. The shortener saves that destination and sends the client a short code.
3. The client uses the code to ask where it leads.
4. The shortener replies `Docs`, and the client arrives there.

The questions are simple: **Can this round trip finish? If the client arrives at `Docs`, did the shortener save `Docs`?**

A model is a small description of this story, not the finished program. By leaving out HTTP, storage engines, authentication, and many users, we can focus on the conversation that matters here.

## 2. Give the story's ideas names

First, name the things the story needs to remember:

```fml
// The two destinations used in this example.
type Destination = Docs | Home

// Keep the code space tiny: this model has one short code.
type Code = DocsLink

// The shortener starts empty, then remembers one destination.
type StoreState = Empty | Stored(Destination)
```

These names make the example easier to follow. They don't say that a real service must only accept two destinations or one code; they keep this particular design question small and understandable.

:::tip[What does “explore every possibility” mean?]
For the behavior we have described, FML follows each allowed next step—such as submitting the request or deciding which waiting message to handle next—instead of checking only a handful of example runs. This is sometimes called exploring exhaustively. The result still covers only the values, participants, messages, and assumptions we put in this model. If a configured limit prevents the search from finishing, FML reports that it could not conclude; it doesn't quietly skip the hard cases and say everything is fine.
:::

## 3. Describe the messages

The client and shortener need to talk. A command says what the client wants; a reply says what happened. The client includes its address so the shortener has somewhere to send its response. It also passes the shortener's own address back so the client can ask a follow-up question:

```fml
type Command =
  Shorten(Actor<Client>, Actor<Shortener>, Destination)
  | Resolve(Actor<Client>, Code)

type Reply =
  Created(Code, Actor<Client>, Actor<Shortener>)
  | Redirect(Destination)
  | Missing
```

`Actor<Client>` and `Actor<Shortener>` are references to participants in this model. A message is not a synchronous function call: sending a reply creates another message for a later turn. That lets us describe the interaction instead of hiding it inside one operation.

## 4. Describe what each part does

When the shortener handles a `Shorten` command, it saves the destination and sends back `Created`. Both happen in the same short turn:

```fml
| Shorten(reply_to, me, destination) -> {
    send(reply_to, Created(DocsLink, reply_to, me)); // send a separate reply
    Stored(destination)                              // keep the destination
  }
```

The client uses that reply to send a `Resolve` command. The full model also describes an empty store, an unknown link, and the other message cases; see the [complete example](https://github.com/leostera/flareml/blob/main/examples/link-shortener.fml).

A handler's state update is not itself a reply. For example, returning `Stored(Docs)` changes the shortener's memory, but the client only learns something if the shortener sends a message. That distinction is an easy place for a design bug to hide.

## 5. Say what should happen

A **property** is a statement about behavior that we want the model to check. FML uses the `property` keyword for these statements. Here are the two questions from our story:

```fml
// Can at least one round trip reach the Docs destination?
property "a link can be shortened and resolved" {
  reachable (exists (client in instances(Client)) {
    client.state == Some(Arrived(Docs))
  })
}

// Whenever a client arrives at Docs, was Docs what the shortener saved?
property "a redirect matches the saved destination" {
  always (forall (client in instances(Client)) {
    client.state == Some(Arrived(Docs)) implies
      (exists (shortener in instances(Shortener)) {
        shortener.state == Some(Stored(Docs))
      })
  })
}
```

Read the first as **“can this happen?”** `reachable` asks whether the described situation can occur. Read the second as **“does this stay true whenever we look?”** `always` checks the starting point and every later point in the model's explored executions.

The complete model also asks whether submitted work eventually finishes. That progress question is conditional: an outside client may choose not to submit a request at all. Learn more about [`property`, reachability, and `always`](/reference/properties/) once these first questions feel familiar.

## 6. Create the participants

An actor declaration describes a kind of participant; it doesn't create one by itself. The check's `main` block starts from an empty model and creates one shortener and one client:

```fml
check ShortenOneLink {
  spawn_bound Shortener = 1
  spawn_bound Client = 1
  mailbox_bound = 1
  main {
    let shortener = spawn(Shortener);
    let client = spawn(Client);

    // This outside request may be submitted once, or not at all.
    inputs { once send(shortener, Shorten(client, shortener, Docs)) }
  }
  fairness { weak runtime.progress }
}
```

`spawn_bound` says how many participants of each kind this example may create; `mailbox_bound` limits how many messages may be waiting at one participant at once. One is enough for each here. `inputs` describes a request from outside the model. It may be submitted once, but the model doesn't assume it must be.

The fairness line says that a participant whose mailbox stays ready won't be ignored forever. It doesn't make the outside client send its request. See [checks and bounds](/reference/checks/) for details when you need them.

## 7. Run the example and look at what happened

Install FlareML and check the complete example:

```sh
cargo install flareml
fml check examples/link-shortener.fml --trace-out /tmp/link-shortener.json
fml replay examples/link-shortener.fml /tmp/link-shortener.json
```

The model finds a round trip, and the destination check holds for the behavior we've described. The replayable trace shows setup creating both participants, the request being submitted, the shortener's reply, the client's follow-up, and the final redirect. This is evidence about our model—not a promise about a future implementation or real network.

Each run also saves its source, configuration, report, and available traces under `.fml/runs/<run-id>/`. If you later edit the example, replay a trace against the saved `model.fml` from that run.

## 8. See how a missing reply changes the story

Remove the `send(reply_to, Created(...))` from the `Shorten` case. The shortener still saves the destination, but the client never learns the code and never asks to resolve it. The progress property now finds a counterexample: a request was submitted, but the client keeps waiting.

That's the point of modeling before implementation: even a tiny omission in the conversation between two parts becomes visible while the design is still easy to change.

## Keep learning

- [Get started](/guide/getting-started/) installs the CLI and checks your first model.
- [Language overview](/guide/language/) introduces the rest of FML.
- [Execution model](/guide/execution-model/) explains setup, messages, turns, and scheduling.
- [Examples](/examples/) contains more small designs, including bugs and repairs.

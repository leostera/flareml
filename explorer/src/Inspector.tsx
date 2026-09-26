import { useState } from "react";
import type { Actor, Metadata, ModelValue, Snapshot } from "./trace/schema";
import { changesBetween, modelText } from "./trace/presentation";

function StateView({ value }: { value: ModelValue | null }) {
  const [limit, setLimit] = useState(50);
  if (value && typeof value === "object" && "Record" in value) {
    const [name, fields] = value.Record;
    const entries = Object.entries(fields);
    return (
      <div className="state-fields">
        <small>State type · {name}</small>
        {entries.slice(0, limit).map(([key, v]) => (
          <div className="state-field" key={key}>
            <span>{key}</span>
            <code>{modelText(v)}</code>
          </div>
        ))}
        {entries.length > limit && (
          <button onClick={() => setLimit(limit + 50)}>Show more fields</button>
        )}
      </div>
    );
  }
  return <pre className="model-value">{modelText(value)}</pre>;
}
export default function Inspector({
  frame,
  previous,
  actor,
  meta,
  endpoint,
  select,
}: {
  frame?: Snapshot;
  previous?: Snapshot;
  actor?: Actor;
  meta?: Metadata;
  endpoint: boolean;
  select: (id: string) => void;
}) {
  const [limit, setLimit] = useState(50);
  if (!frame)
    return (
      <aside className="inspector">
        <p>Loading transition…</p>
      </aside>
    );
  const changes = changesBetween(previous, frame).sort(
    (a, b) => Number(b.fields.length > 0) - Number(a.fields.length > 0),
  );
  const event = frame.event;
  return (
    <aside className="inspector debugger">
      {endpoint && meta && (
        <section
          className={
            meta.kind === "Cover"
              ? "property-detail"
              : "property-detail violation"
          }
        >
          <h2>
            {meta.kind === "Cover" ? "Reached property" : "Violated property"}
          </h2>
          <strong>{meta.claim}</strong>
          <pre>{meta.property.text}</pre>
          <p>
            {meta.kind === "Cover"
              ? `This execution reaches the property at snapshot ${frame.index}.`
              : meta.kind === "Invariant"
                ? `The invariant is false at snapshot ${frame.index}.`
                : meta.loop_start !== null
                  ? `This property fails over the infinite execution: snapshot ${frame.index} closes a loop back to ${meta.loop_start}. The repeated suffix, not just this final state, is the counterexample.`
                  : "This temporal property fails over the saved execution, not necessarily in the final state alone."}
          </p>
          {meta.clause !== null && (
            <p>Failing temporal clause: {meta.clause + 1}</p>
          )}
          <small>
            Line {meta.property.line} · replay-validated evidence, not an
            inferred causal explanation
          </small>
        </section>
      )}
      <section className="transition-panel">
        <h2>
          {frame.index === 0
            ? "Initial setup"
            : `Transition ${frame.index - 1} → ${frame.index}`}
        </h2>
        <p className="transition-summary">
          {event.kind === "setup"
            ? "Deterministic initial population"
            : event.kind === "process"
              ? `${event.actor?.replace("actor:", "").replaceAll(":", " #")} processed a message`
              : event.kind === "submit"
                ? "External input submitted"
                : "Stutter: no system action"}
        </p>
        {event.consumed && (
          <div className="event-line">
            <small>Received</small>
            <code>{modelText(event.consumed.payload)}</code>
          </div>
        )}
        <h3>What changed</h3>
        {!changes.length && <p>No state or mailbox changes.</p>}
        {changes.slice(0, limit).map((change) => (
          <div className="change-card" key={change.actor.id}>
            <button onClick={() => select(change.actor.id)}>
              {change.actor.name} #{change.actor.slot}
              {change.created ? " · created" : ""}
            </button>
            {change.fields.map((field, i) => (
              <div className="field-change" key={i}>
                <span>{field.field}</span>
                <div>
                  <del>{modelText(field.before)}</del>
                  <span aria-hidden="true"> → </span>
                  <ins>{modelText(field.after)}</ins>
                </div>
              </div>
            ))}
            {change.fields.length === 100 && (
              <small>First 100 field changes shown</small>
            )}
            {change.queueChanged && (
              <p>
                Mailbox: {change.queuedBefore} → {change.actor.mailbox.length}{" "}
                pending
                {change.queuedBefore === change.actor.mailbox.length
                  ? " (envelopes changed)"
                  : ""}
              </p>
            )}
          </div>
        ))}
        {changes.length > limit && (
          <button onClick={() => setLimit(limit + 50)}>
            Show more changed entities
          </button>
        )}
        {event.sends.length > 0 && (
          <>
            <h3>Sent messages</h3>
            {event.sends.slice(0, 50).map((send) => (
              <div className="event-line" key={send.id}>
                <button onClick={() => select(send.target)}>
                  {send.target.replace("actor:", "").replaceAll(":", " #")}
                </button>
                <code>{modelText(send.payload)}</code>
              </div>
            ))}
            {event.sends.length > 50 && (
              <p>First 50 of {event.sends.length} sends shown.</p>
            )}
          </>
        )}
        {event.choices.length > 0 && (
          <>
            <h3>Choices</h3>
            {event.choices.slice(0, 50).map((c) => (
              <div className="event-line" key={c.encounter}>
                <small>
                  Choice {c.encounter + 1} · candidate {c.candidate + 1}
                </small>
                <code>{modelText(c.value)}</code>
              </div>
            ))}
          </>
        )}
        {event.spawns.length > 0 && (
          <p>
            Created {event.spawns.length}{" "}
            {event.spawns.length === 1 ? "entity" : "entities"}.
          </p>
        )}
        {event.source && (
          <details>
            <summary>Handler source · line {event.source.line}</summary>
            <pre>{event.source.text}</pre>
          </details>
        )}
        <details>
          <summary>Raw transition evidence</summary>
          <pre>{JSON.stringify(event, null, 2)}</pre>
        </details>
      </section>
      <section className="entity-panel">
        <h2>{actor ? `${actor.name} #${actor.slot}` : "Selected entity"}</h2>
        {actor ? (
          <>
            <small>Actor type · {actor.name}</small>
            <h3>Current state</h3>
            {actor.stateful ? (
              <StateView value={actor.state} />
            ) : (
              <p>Stateless component</p>
            )}
            <h3>Mailbox · {actor.mailbox.length}</h3>
            {!actor.mailbox.length && <p>Empty</p>}
            {actor.mailbox.slice(0, 50).map((m, i) => (
              <details className="message-detail" key={m.id}>
                <summary>
                  {i === 0 ? "Head" : `#${i}`} · {modelText(m.payload)}
                </summary>
                <StateView value={m.payload} />
                <small>
                  {m.id} · sent at line {m.source.line}
                </small>
                <pre>{m.source.text}</pre>
              </details>
            ))}
            {actor.mailbox.length > 50 && (
              <p>First 50 queued envelopes shown.</p>
            )}
            <details>
              <summary>Raw entity evidence</summary>
              <pre>{JSON.stringify(actor, null, 2)}</pre>
            </details>
          </>
        ) : (
          <p>Select an entity in the graph, Entities tab, or change list.</p>
        )}
      </section>
    </aside>
  );
}

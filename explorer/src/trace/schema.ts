import { Schema } from "effect";
// Presentation schema authority; CI decodes fixtures from real authoritative replay.
const Nat = Schema.NonNegativeInt.pipe(
  Schema.lessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
export const Location = Schema.Struct({
  start: Nat,
  end: Nat,
  line: Nat,
  column: Nat,
  text: Schema.String,
});
export type Location = typeof Location.Type;
export type ModelValue =
  | "Unit"
  | { readonly Bool: boolean }
  | { readonly Int: string }
  | { readonly String: string }
  | { readonly Identity: number }
  | { readonly Input: number }
  | { readonly Variant: readonly [string, readonly ModelValue[]] }
  | { readonly Record: readonly [string, Readonly<Record<string, ModelValue>>] }
  | { readonly List: readonly ModelValue[] }
  | { readonly Address: readonly [string, ModelValue] }
  | { readonly Instance: readonly [string, number] }
  | { readonly Message: readonly [string, number] };
export const Value: Schema.Schema<ModelValue> = Schema.suspend(() =>
  Schema.Union(
    Schema.Literal("Unit"),
    Schema.Struct({ Bool: Schema.Boolean }),
    Schema.Struct({ Int: Schema.String.pipe(Schema.pattern(/^-?\d+$/)) }),
    Schema.Struct({ String: Schema.String }),
    Schema.Struct({ Identity: Nat }),
    Schema.Struct({ Input: Nat }),
    Schema.Struct({
      Variant: Schema.Tuple(Schema.String, Schema.Array(Value)),
    }),
    Schema.Struct({
      Record: Schema.Tuple(
        Schema.String,
        Schema.Record({ key: Schema.String, value: Value }),
      ),
    }),
    Schema.Struct({ List: Schema.Array(Value) }),
    Schema.Struct({ Address: Schema.Tuple(Schema.String, Value) }),
    Schema.Struct({ Instance: Schema.Tuple(Schema.String, Nat) }),
    Schema.Struct({ Message: Schema.Tuple(Schema.String, Nat) }),
  ),
);
const Envelope = Schema.Struct({
  id: Schema.String,
  payload: Value,
  input: Schema.NullOr(Nat),
  observation: Schema.NullOr(Nat),
  source: Location,
});
const Send = Schema.Struct({
  id: Schema.String,
  target: Schema.String,
  payload: Value,
  source: Location,
});
const Choice = Schema.Struct({
  encounter: Nat,
  candidate: Nat,
  value: Value,
  source: Location,
  calls: Schema.Array(Location),
});
const Spawn = Schema.Struct({
  actor: Schema.String,
  initial: Value,
  arguments: Schema.Array(Value),
  source: Location,
  calls: Schema.Array(Location),
});
const Kind = Schema.Literal("setup", "submit", "process", "stutter");
const Event = Schema.Struct({
  kind: Kind,
  actor: Schema.NullOr(Schema.String),
  consumed: Schema.NullOr(Envelope),
  sends: Schema.Array(Send),
  choices: Schema.Array(Choice),
  spawns: Schema.Array(Spawn),
  source: Schema.NullOr(Location),
});
export const Metadata = Schema.Struct({
  property: Location,
  clause: Schema.NullOr(Nat),
  schema_version: Schema.Literal(1),
  check: Schema.String,
  claim: Schema.String,
  kind: Schema.Literal("Cover", "Invariant", "Property"),
  source_hash: Schema.String,
  source: Schema.String,
  tool_version: Schema.String,
  trace_version: Nat,
  snapshots: Nat,
  loop_start: Schema.NullOr(Nat),
  fair: Schema.Boolean,
  spawn_bounds: Schema.Record({ key: Schema.String, value: Nat }),
  mailbox_bound: Schema.NullOr(Nat),
  message_bound: Schema.NullOr(Nat),
});
export type Metadata = typeof Metadata.Type;
export const Actor = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  slot: Nat,
  spawn_order: Nat,
  label: Schema.String,
  stateful: Schema.Boolean,
  state: Schema.NullOr(Value),
  mailbox: Schema.Array(Envelope),
});
export type Actor = typeof Actor.Type;
export const Snapshot = Schema.Struct({
  flows: Schema.Array(
    Schema.Struct({ source: Schema.String, target: Schema.String, count: Nat }),
  ),
  index: Nat,
  actors: Schema.Array(Actor),
  event: Event,
  inputs: Schema.Array(
    Schema.Struct({
      target: Value,
      payload: Value,
      source: Schema.Struct({ start: Nat, end: Nat }),
    }),
  ),
  submitted: Schema.Array(Schema.Boolean),
  processed: Schema.Array(Schema.Boolean),
  messages: Schema.Record({
    key: Schema.String,
    value: Schema.Array(
      Schema.Struct({
        payload: Value,
        target: Value,
        external: Schema.Boolean,
        processed: Schema.Boolean,
      }),
    ),
  }),
});
export type Snapshot = typeof Snapshot.Type;
export const Summaries = Schema.Struct({
  total: Nat,
  items: Schema.Array(
    Schema.Struct({
      index: Nat,
      kind: Kind,
      actor: Schema.NullOr(Schema.String),
      choices: Nat,
      spawns: Nat,
    }),
  ),
});
export type Summaries = typeof Summaries.Type;
export const ExecutionTree = Schema.Struct({
  nodes: Schema.Array(
    Schema.Struct({
      id: Nat,
      parent: Schema.NullOr(Nat),
      step: Nat,
      trace: Nat,
      kind: Kind,
      actor: Schema.NullOr(Schema.String),
      choices: Schema.Array(Choice),
      ends: Schema.Array(Nat),
    }),
  ),
  traces: Schema.Array(
    Schema.Struct({
      id: Nat,
      claim: Schema.String,
      violation: Schema.Boolean,
      loop_start: Schema.NullOr(Nat),
      path: Schema.Array(Nat),
    }),
  ),
});
export type ExecutionTree = typeof ExecutionTree.Type;

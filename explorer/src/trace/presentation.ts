import type { ModelValue, Snapshot } from "./schema";
export function modelText(
  value: ModelValue | null | undefined,
  depth = 0,
): string {
  if (value === undefined) return "not present";
  if (value === null) return "no owned state";
  if (value === "Unit") return "()";
  if (depth > 12) return "… (depth limit)";
  const text = (v: ModelValue) => modelText(v, depth + 1);
  const list = (values: readonly ModelValue[]) =>
    values.slice(0, 50).map(text).join(", ") +
    (values.length > 50 ? `, … (${values.length - 50} more)` : "");
  if ("Int" in value) return value.Int;
  if ("Bool" in value) return String(value.Bool);
  if ("String" in value)
    return JSON.stringify(
      value.String.length > 300
        ? value.String.slice(0, 300) + "… (preview)"
        : value.String,
    );
  if ("Identity" in value) return `#${value.Identity}`;
  if ("Address" in value)
    return `${value.Address[0]} ${text(value.Address[1])}`;
  if ("Variant" in value)
    return (
      value.Variant[0] +
      (value.Variant[1].length ? `(${list(value.Variant[1])})` : "")
    );
  if ("Record" in value) {
    const entries = Object.entries(value.Record[1]);
    return `${value.Record[0]} { ${entries
      .slice(0, 50)
      .map(([k, v]) => `${k}: ${text(v)}`)
      .join(", ")}${entries.length > 50 ? ", … (more fields)" : ""} }`;
  }
  if ("List" in value) return `[${list(value.List)}]`;
  if ("Instance" in value) return `${value.Instance[0]} #${value.Instance[1]}`;
  if ("Input" in value) return `input #${value.Input}`;
  return `message ${value.Message[0]} #${value.Message[1]}`;
}
export type FieldChange = {
  field: string;
  before: ModelValue | null | undefined;
  after: ModelValue | null | undefined;
};
export function stateDiff(
  before: ModelValue | null | undefined,
  after: ModelValue | null | undefined,
  field = "State",
  out: FieldChange[] = [],
): FieldChange[] {
  if (out.length >= 100 || JSON.stringify(before) === JSON.stringify(after))
    return out;
  if (
    before &&
    after &&
    typeof before === "object" &&
    typeof after === "object" &&
    "Record" in before &&
    "Record" in after &&
    before.Record[0] === after.Record[0]
  ) {
    for (const key of new Set([
      ...Object.keys(before.Record[1]),
      ...Object.keys(after.Record[1]),
    ]))
      stateDiff(
        before.Record[1][key],
        after.Record[1][key],
        field === "State" ? key : `${field}.${key}`,
        out,
      );
  } else out.push({ field, before, after });
  return out;
}
export function changesBetween(before: Snapshot | undefined, after: Snapshot) {
  const previous = new Map(before?.actors.map((a) => [a.id, a]) ?? []);
  return after.actors.flatMap((actor) => {
    const old = previous.get(actor.id);
    const fields = actor.stateful ? stateDiff(old?.state, actor.state) : [];
    const queueChanged =
      JSON.stringify(old?.mailbox.map((m) => m.id) ?? []) !==
      JSON.stringify(actor.mailbox.map((m) => m.id));
    return !old || fields.length || queueChanged
      ? [
          {
            actor,
            created: !old,
            fields,
            queueChanged,
            queuedBefore: old?.mailbox.length ?? 0,
          },
        ]
      : [];
  });
}

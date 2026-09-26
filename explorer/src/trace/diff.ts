export type Change = { path: string; before: unknown; after: unknown };
export function diff(
  before: unknown,
  after: unknown,
  path = "$",
  changes: Change[] = [],
): Change[] {
  if (changes.length >= 200 || JSON.stringify(before) === JSON.stringify(after))
    return changes;
  if (
    before !== null &&
    after !== null &&
    typeof before === "object" &&
    typeof after === "object" &&
    Array.isArray(before) === Array.isArray(after)
  ) {
    const a = before as Record<string, unknown>,
      b = after as Record<string, unknown>;
    for (const key of new Set([...Object.keys(a), ...Object.keys(b)])) {
      if (changes.length >= 200) break;
      diff(a[key], b[key], `${path}.${key}`, changes);
    }
  } else {
    changes.push({ path, before, after });
  }
  return changes;
}
export const pretty = (value: unknown): string =>
  value === undefined ? "(absent)" : JSON.stringify(value, null, 2);
export function boundedStep(index: number, count: number) {
  return Math.max(0, Math.min(count - 1, index));
}

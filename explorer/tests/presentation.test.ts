import { test, expect } from "bun:test";
import {
  modelText,
  stateDiff,
  changesBetween,
} from "../src/trace/presentation";
import { Schema } from "effect";
import { Snapshot, type ModelValue } from "../src/trace/schema";
import fixture from "./fixtures/lossless.json";
test("model values use source-language names without serializer wrappers", () => {
  const state: ModelValue = {
    Record: [
      "PaymentState",
      {
        charges: { Int: "9223372036854775807" },
        id: { Variant: ["PaymentA", []] },
        reply: { Address: ["Gateway", { Identity: 0 }] },
      },
    ],
  };
  expect(modelText(state)).toBe(
    "PaymentState { charges: 9223372036854775807, id: PaymentA, reply: Gateway #0 }",
  );
  expect(
    modelText({ Variant: ["Some", [{ Int: "-9223372036854775808" }]] }),
  ).toBe("Some(-9223372036854775808)");
  expect(modelText("Unit")).toBe("()");
});
test("field diff preserves names, exact integers, additions, and type changes", () => {
  const before: ModelValue = {
    Record: ["PaymentState", { charges: { Int: "1" } }],
  };
  const after: ModelValue = {
    Record: [
      "PaymentState",
      { charges: { Int: "2" }, ledger: { Variant: ["Recorded", []] } },
    ],
  };
  expect(stateDiff(before, after).map((c) => c.field)).toEqual([
    "charges",
    "ledger",
  ]);
  expect(stateDiff(before, after)[1].before).toBeUndefined();
  expect(
    stateDiff({ Int: "9223372036854775807" }, { Int: "9223372036854775806" }),
  ).toHaveLength(1);
  expect(stateDiff(before, before)).toEqual([]);
});
test("snapshot comparison is identity-based and detects equal-length queue replacement", () => {
  const initial = Schema.decodeUnknownSync(Snapshot)(fixture.snapshots[0]);
  const after = {
    ...initial,
    actors: initial.actors.map((a) => ({
      ...a,
      mailbox: a.mailbox.map((m) => ({ ...m, id: "new-envelope" })),
    })),
  };
  const changed = changesBetween(initial, after);
  expect(changed).toHaveLength(1);
  expect(changed[0].fields).toHaveLength(0);
  expect(changed[0].queueChanged).toBe(true);
  expect(
    changesBetween(initial, {
      ...initial,
      actors: [...initial.actors].reverse(),
    }),
  ).toHaveLength(0);
});

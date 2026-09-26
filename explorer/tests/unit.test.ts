import { expect, test } from "bun:test";
import { Effect, Schema } from "effect";
import { Metadata, Snapshot, Summaries, Value } from "../src/trace/schema";
import { boundedStep, diff } from "../src/trace/diff";
import fixture from "./fixtures/lossless.json";

test("Rust-generated projection conforms to Effect Schema", async () => {
  const data = await Effect.runPromise(
    Effect.all({
      metadata: Schema.decodeUnknown(Metadata)(fixture.metadata),
      summaries: Schema.decodeUnknown(Summaries)(fixture.summaries),
      snapshots: Effect.forEach(fixture.snapshots, (snapshot) =>
        Schema.decodeUnknown(Snapshot)(snapshot),
      ),
    }),
  );
  expect(data.metadata.schema_version).toBe(1);
  expect(data.snapshots[0].actors[0].state).toEqual({
    Int: "9223372036854775807",
  });
  expect(data.snapshots[1].event.consumed?.id).toBe(
    data.snapshots[0].actors[0].mailbox[0].id,
  );
});
test("unsafe numeric model values are rejected instead of rounded", async () => {
  const exit = await Effect.runPromiseExit(
    Schema.decodeUnknown(Value)({ Int: 9223372036854775807 }),
  );
  expect(exit._tag).toBe("Failure");
  expect(
    await Effect.runPromise(
      Schema.decodeUnknown(Value)({ Int: "-9223372036854775808" }),
    ),
  ).toEqual({ Int: "-9223372036854775808" });
});
test("structural differences retain exact strings, absent fields, and queue positions", () => {
  expect(
    diff({ Int: "9223372036854775807" }, { Int: "9223372036854775806" }),
  ).toEqual([
    {
      path: "$.Int",
      before: "9223372036854775807",
      after: "9223372036854775806",
    },
  ]);
  expect(diff({ a: 1 }, { a: 1 })).toEqual([]);
  expect(diff({}, { a: null })[0].before).toBeUndefined();
  expect(diff([1, 2], [2, 1])).toHaveLength(2);
});
test("navigation and difference rendering are bounded", () => {
  expect(boundedStep(-1, 10)).toBe(0);
  expect(boundedStep(20, 10)).toBe(9);
  expect(diff(Array(1000).fill(false), Array(1000).fill(true))).toHaveLength(
    200,
  );
});
test("unknown schema versions fail closed", async () => {
  expect(
    (
      await Effect.runPromiseExit(
        Schema.decodeUnknown(Metadata)({
          ...fixture.metadata,
          schema_version: 99,
        }),
      )
    )._tag,
  ).toBe("Failure");
});

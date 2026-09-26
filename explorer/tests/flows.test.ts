import { test, expect } from "bun:test";
import { Schema } from "effect";
import { Snapshot } from "../src/trace/schema";
import { messageEdges } from "../src/MessageFlow";
import fixture from "./fixtures/lossless.json";

test("requests and replies share one edge and reverse only the active direction", () => {
  const frame = Schema.decodeUnknownSync(Snapshot)(fixture.snapshots[0]);
  const a = frame.actors[0].id,
    b = "actor:B:0";
  const visible = new Set([a, b]);
  const request = messageEdges(
    {
      ...frame,
      flows: [
        { source: a, target: b, count: 2 },
        { source: b, target: a, count: 0 },
      ],
    },
    visible,
  );
  const reply = messageEdges(
    {
      ...frame,
      flows: [
        { source: a, target: b, count: 0 },
        { source: b, target: a, count: 1 },
      ],
    },
    visible,
  );
  expect(request).toHaveLength(1);
  expect(reply).toHaveLength(1);
  expect(request[0].id).toBe(reply[0].id);
  expect(request[0].source).toBe(reply[0].source);
  expect(request[0].target).toBe(reply[0].target);
  expect(request[0].label).toBe("2 messages");
  expect(request[0].markerEnd).toBeDefined();
  expect(request[0].markerStart).toBeUndefined();
  expect(reply[0].markerStart).toBeDefined();
  expect(reply[0].markerEnd).toBeUndefined();
  expect(reply[0].className).toContain("reverse-flow");
  expect(reply[0].data?.recipient).toBe(a);
  const idle = messageEdges(
    {
      ...frame,
      flows: [
        { source: a, target: b, count: 0 },
        { source: b, target: a, count: 0 },
      ],
    },
    visible,
  );
  expect(idle).toHaveLength(1);
  expect(idle[0].animated).toBe(false);
  expect(idle[0].markerStart).toBeUndefined();
  expect(idle[0].markerEnd).toBeUndefined();
});
test("self sends remain an explicit loop; hidden endpoints are never redirected", () => {
  const frame = Schema.decodeUnknownSync(Snapshot)(fixture.snapshots[0]);
  const source = frame.actors[0].id;
  const self = messageEdges(
    { ...frame, flows: [{ source, target: source, count: 1 }] },
    new Set([source]),
  );
  expect(self).toHaveLength(1);
  expect(self[0].source).toBe(self[0].target);
  expect(self[0].markerEnd).toBeDefined();
  expect(
    messageEdges(
      { ...frame, flows: [{ source, target: "hidden", count: 1 }] },
      new Set([source]),
    ),
  ).toEqual([]);
});

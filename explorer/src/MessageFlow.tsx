import {
  BaseEdge,
  MarkerType,
  getBezierPath,
  type Edge,
  type EdgeProps,
} from "@xyflow/react";
import type { Snapshot } from "./trace/schema";
import { modelText } from "./trace/presentation";

export function messageEdges(
  frame: Snapshot,
  visible: ReadonlySet<string>,
): Edge[] {
  // One stable geometric connection per pair. Replies reverse the marker and
  // animation on that same connection; this does not infer RPC correlation.
  const pairs = new Map<
    string,
    { source: string; target: string; routes: Snapshot["flows"][number][] }
  >();
  const birth = new Map(
    frame.actors.map((actor) => [actor.id, actor.spawn_order]),
  );
  for (const flow of frame.flows) {
    if (!visible.has(flow.source) || !visible.has(flow.target)) continue;
    const pairIds = [flow.source, flow.target].sort();
    const id = JSON.stringify(pairIds);
    const [source, target] = pairIds.sort(
      (a, b) =>
        (birth.get(a) ?? Number.MAX_SAFE_INTEGER) -
        (birth.get(b) ?? Number.MAX_SAFE_INTEGER),
    );
    const pair = pairs.get(id) ?? { source, target, routes: [] };
    pair.routes.push(flow);
    pairs.set(id, pair);
  }
  return [...pairs].map(([id, { source, target, routes }]) => {
    const active = routes.filter((r) => r.count > 0);
    const count = active.reduce((sum, r) => sum + r.count, 0);
    const forward = active.some((r) => r.source === source),
      reverse = active.some((r) => r.source !== source);
    const recipient = active[0]?.target ?? target;
    const color = count ? "#527c9f" : "#b4c0cc";
    const payload = frame.event.sends.find(
      (s) => s.target === recipient,
    )?.payload;
    const preview = payload ? modelText(payload) : "message";
    const marker = {
      type: MarkerType.ArrowClosed,
      color,
      orient: "auto-start-reverse",
    };
    return {
      id,
      source,
      target,
      type: "message",
      data: { recipient },
      animated: count > 0,
      className: count
        ? `message-flow active-flow${reverse && !forward ? " reverse-flow" : ""}`
        : "message-flow past-flow",
      label: count
        ? count > 1
          ? `${count} messages`
          : preview.length > 48
            ? preview.slice(0, 48) + "…"
            : preview
        : undefined,
      ariaLabel: count
        ? `${active[0].source} to ${recipient}: ${count} sends in this transition`
        : `${source} and ${target}: previously observed route`,
      markerEnd: forward ? marker : undefined,
      markerStart: reverse ? marker : undefined,
      style: {
        stroke: color,
        strokeWidth: count ? 2 : 1.2,
        strokeDasharray: count ? undefined : "3 6",
      },
      labelStyle: { fill: "#45647f", fontSize: 10 },
      labelBgStyle: { fill: "#fff" },
      labelBgPadding: [6, 4] as [number, number],
      labelBgBorderRadius: 4,
    };
  });
}
export function MessageEdge(props: EdgeProps) {
  const { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition } =
    props;
  const [path, labelX, labelY] =
    props.source === props.target
      ? ([
          `M ${sourceX} ${sourceY} C ${sourceX + 90} ${sourceY - 150}, ${targetX - 90} ${targetY - 150}, ${targetX} ${targetY}`,
          (sourceX + targetX) / 2,
          Math.min(sourceY, targetY) - 112,
        ] as const)
      : Math.abs(sourceY - targetY) < 30 && targetX - sourceX > 200
        ? ([
            `M ${sourceX} ${sourceY} C ${sourceX + 100} ${sourceY - 130}, ${targetX - 100} ${targetY - 130}, ${targetX} ${targetY}`,
            (sourceX + targetX) / 2,
            Math.min(sourceY, targetY) - 97,
          ] as const)
        : getBezierPath({
            sourceX,
            sourceY,
            targetX,
            targetY,
            sourcePosition,
            targetPosition,
          });
  return (
    <BaseEdge
      id={props.id}
      path={path}
      markerStart={props.markerStart}
      markerEnd={props.markerEnd}
      style={props.style}
      label={props.label}
      labelX={labelX}
      labelY={labelY}
      labelStyle={props.labelStyle}
      labelBgStyle={props.labelBgStyle}
      labelBgPadding={props.labelBgPadding}
      labelBgBorderRadius={props.labelBgBorderRadius}
    />
  );
}
export const messageEdgeTypes = { message: MessageEdge };

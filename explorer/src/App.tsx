import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type DependencyList,
} from "react";
import { Effect, Fiber } from "effect";
import {
  Background,
  Controls,
  ReactFlow,
  applyNodeChanges,
  type Node,
} from "@xyflow/react";
import type { Provider, ExplorerError } from "./providers/native";
import type { ExecutionTree, Metadata, Snapshot } from "./trace/schema";
import { boundedStep } from "./trace/diff";
import { changesBetween } from "./trace/presentation";
import Inspector from "./Inspector";

function useRequest<A>(
  task: () => Effect.Effect<A, ExplorerError>,
  success: (a: A) => void,
  failure: (message: string) => void,
  dependencies: DependencyList,
) {
  useEffect(() => {
    const fiber = Effect.runFork(
      task().pipe(
        Effect.match({
          onSuccess: success,
          onFailure: (e) => failure(e.message),
        }),
      ),
    );
    return () => {
      Effect.runFork(Fiber.interrupt(fiber));
    };
  }, dependencies);
}
function Graph({
  frame,
  changed,
  selected,
  select,
  positions,
}: {
  frame: Snapshot;
  changed: Set<string>;
  selected: string | null;
  select: (id: string) => void;
  positions: Map<string, { x: number; y: number }>;
}) {
  const [nodes, setNodes] = useState<Node[]>([]);
  useEffect(() => {
    const visible = frame.actors.slice(0, 200);
    const chosen = frame.actors.find((a) => a.id === selected);
    if (chosen && !visible.includes(chosen)) visible.splice(199, 1, chosen);
    setNodes(
      visible.map((actor) => {
        if (!positions.has(actor.id)) {
          const i = positions.size;
          positions.set(actor.id, {
            x: (i % 3) * 250,
            y: Math.floor(i / 3) * 150,
          });
        }
        return {
          id: actor.id,
          selected: actor.id === selected,
          position: positions.get(actor.id)!,
          className: changed.has(actor.id) ? "changed-entity" : "",
          data: {
            label: (
              <>
                <strong>
                  {actor.name} #{actor.slot}
                </strong>
                <small>
                  {actor.mailbox.length} pending
                  {changed.has(actor.id) ? " · changed" : ""}
                </small>
              </>
            ),
          },
        };
      }),
    );
  }, [frame, selected, changed, positions]);
  return (
    <ReactFlow
      nodes={nodes}
      edges={[]}
      fitView
      fitViewOptions={{ maxZoom: 1.2 }}
      nodesConnectable={false}
      deleteKeyCode={null}
      onNodeClick={(_, node) => select(node.id)}
      onNodesChange={(changes) => {
        for (const c of changes)
          if (c.type === "position" && c.position)
            positions.set(c.id, c.position);
        setNodes((n) => applyNodeChanges(changes, n));
      }}
    >
      <Background gap={28} color="#e1e4e8" />
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
function orderedNodes(tree: ExecutionTree) {
  const children = new Map<number | null, number[]>();
  for (const node of tree.nodes) {
    const list = children.get(node.parent) ?? [];
    list.push(node.id);
    children.set(node.parent, list);
  }
  const stack = (children.get(null) ?? [])
    .slice()
    .reverse()
    .map((id) => ({ id, depth: 0 }));
  const rows: { id: number; depth: number }[] = [];
  while (stack.length) {
    const row = stack.pop()!;
    rows.push(row);
    for (const id of (children.get(row.id) ?? []).slice().reverse())
      stack.push({ id, depth: row.depth + 1 });
  }
  return rows;
}
export default function App({ provider }: { provider: Provider }) {
  const [tree, setTree] = useState<ExecutionTree>();
  const [trace, setTrace] = useState(0),
    [index, setIndex] = useState(0);
  const [meta, setMeta] = useState<Metadata>(),
    [frame, setFrame] = useState<Snapshot>(),
    [previous, setPrevious] = useState<Snapshot>();
  const [error, setError] = useState(""),
    [selected, setSelected] = useState<string | null>(null);
  const [info, setInfo] = useState(false),
    [endpoint, setEndpoint] = useState(false);
  const [tab, setTab] = useState<"traces" | "entities">("traces"),
    [filter, setFilter] = useState("");
  const [treeOffset, setTreeOffset] = useState(0),
    [entityLimit, setEntityLimit] = useState(100);
  const positions = useRef(new Map<string, { x: number; y: number }>());
  useRequest(() => provider.tree(), setTree, setError, [provider]);
  useRequest(
    () =>
      Effect.sync(() => setMeta(undefined)).pipe(
        Effect.zipRight(provider.metadata(trace)),
      ),
    setMeta,
    setError,
    [provider, trace],
  );
  useRequest(
    () =>
      Effect.sync(() => {
        setFrame(undefined);
        setPrevious(undefined);
        setError("");
      }).pipe(
        Effect.zipRight(
          Effect.all(
            [
              provider.snapshot(index, trace),
              index
                ? provider.snapshot(index - 1, trace)
                : Effect.succeed(undefined),
            ],
            { concurrency: "unbounded" },
          ),
        ),
      ),
    ([current, before]) => {
      setFrame(current);
      setPrevious(before);
    },
    setError,
    [provider, index, trace],
  );
  const execution = tree?.traces[trace];
  const currentNode = execution?.path[index];
  const last = execution ? execution.path.length - 1 : 0;
  const rows = useMemo(() => (tree ? orderedNodes(tree) : []), [tree]);
  const red = useMemo(
    () =>
      new Set(
        tree?.traces.filter((t) => t.violation).flatMap((t) => t.path) ?? [],
      ),
    [tree],
  );
  const changed = useMemo(
    () =>
      new Set(
        frame ? changesBetween(previous, frame).map((c) => c.actor.id) : [],
      ),
    [frame, previous],
  );
  useEffect(() => {
    const position = rows.findIndex((r) => r.id === currentNode);
    if (position >= 0) setTreeOffset(Math.floor(position / 200) * 200);
  }, [rows, currentNode]);
  useEffect(() => {
    document
      .querySelector('.execution-tree [aria-current="step"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [currentNode, treeOffset, tab]);
  function navigate(next: number) {
    if (execution) setIndex(boundedStep(next, execution.path.length));
    setEndpoint(false);
  }
  useEffect(() => {
    function key(e: KeyboardEvent) {
      if (
        e.target instanceof HTMLElement &&
        e.target.closest("input,textarea,select,button")
      )
        return;
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        e.preventDefault();
        navigate(index + (e.key === "ArrowRight" ? 1 : -1));
      }
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [index, execution]);
  const actor = frame?.actors.find((a) => a.id === selected);
  const entities =
    frame?.actors.filter((a) =>
      `${a.name} #${a.slot}`.toLowerCase().includes(filter.toLowerCase()),
    ) ?? [];
  function select(id: string) {
    setSelected(id);
  }
  function choose(node: number, preferred?: number) {
    if (!tree) return;
    const target =
      preferred ??
      (execution?.path.includes(node) ? trace : tree.nodes[node].trace);
    setTrace(target);
    setIndex(tree.nodes[node].step);
    setEndpoint(preferred !== undefined);
    setInfo(false);
  }
  return (
    <div className="app">
      <header>
        <span className="brand">flareml</span>
        <span>{meta?.check ?? "Trace explorer"}</span>
        <button
          aria-label="Evidence information"
          onClick={() => setInfo(!info)}
        >
          About this evidence
        </button>
      </header>
      {info && (
        <div className="evidence-note">
          Replay-validated saved witnesses, not the complete search graph. Red
          paths lead to property violations. Normal endpoints are not deadlock
          findings.{" "}
          {meta?.fair
            ? "Weak mailbox progress is assumed."
            : "No scheduling fairness is assumed."}
          <button onClick={() => setInfo(false)}>Close</button>
        </div>
      )}
      {error && <div role="alert">{error}</div>}
      <main>
        <aside className="executions">
          <div
            className="nav-tabs"
            role="tablist"
            aria-label="Explorer navigation"
          >
            <button
              role="tab"
              aria-selected={tab === "traces"}
              onClick={() => setTab("traces")}
            >
              Traces
            </button>
            <button
              role="tab"
              aria-selected={tab === "entities"}
              onClick={() => setTab("entities")}
            >
              Entities
            </button>
          </div>
          {tab === "traces" ? (
            <>
              <h2>
                Saved executions <small>{tree?.traces.length ?? 0}</small>
              </h2>
              <div
                className="execution-tree"
                role="tree"
                aria-label="Saved executions"
              >
                {rows
                  .slice(treeOffset, treeOffset + 200)
                  .map(({ id, depth }) => {
                    const node = tree!.nodes[id];
                    return (
                      <div
                        key={id}
                        style={{ paddingLeft: Math.min(depth, 8) * 12 }}
                      >
                        <button
                          role="treeitem"
                          aria-level={depth + 1}
                          aria-current={id === currentNode ? "step" : undefined}
                          className={`${red.has(id) ? "violation-path" : ""} ${id === currentNode ? "current" : ""}`}
                          onClick={() => choose(id)}
                        >
                          <span className="branch-dot" />
                          <span>
                            {node.kind === "setup"
                              ? "Initial state"
                              : (node.actor
                                  ?.replace("actor:", "")
                                  .replaceAll(":", " · ") ?? node.kind)}
                            {node.choices.length > 0 && (
                              <small>
                                choice{" "}
                                {node.choices
                                  .map((c) => c.candidate + 1)
                                  .join(", ")}
                              </small>
                            )}
                          </span>
                        </button>
                        {node.ends.map((id) => {
                          const end = tree!.traces[id];
                          return (
                            <button
                              key={id}
                              className={`endpoint ${end.violation ? "violation" : ""}`}
                              onClick={() => choose(node.id, id)}
                            >
                              {end.loop_start !== null
                                ? "↻ "
                                : end.violation
                                  ? "× "
                                  : "✓ "}
                              {end.claim}
                            </button>
                          );
                        })}
                      </div>
                    );
                  })}
              </div>
              {rows.length > 200 && (
                <div className="paging">
                  <button
                    disabled={!treeOffset}
                    onClick={() => setTreeOffset(treeOffset - 200)}
                  >
                    Previous
                  </button>
                  <span>
                    {treeOffset + 1}–{Math.min(treeOffset + 200, rows.length)}
                  </span>
                  <button
                    disabled={treeOffset + 200 >= rows.length}
                    onClick={() => setTreeOffset(treeOffset + 200)}
                  >
                    Next
                  </button>
                </div>
              )}
              <p className="scope-note">Saved witnesses only</p>
            </>
          ) : (
            <>
              <h2>Entities at snapshot {index}</h2>
              <input
                className="entity-filter"
                aria-label="Find entity"
                placeholder="Filter entities…"
                value={filter}
                onChange={(e) => {
                  setFilter(e.target.value);
                  setEntityLimit(100);
                }}
              />
              <div className="entity-list">
                {entities.slice(0, entityLimit).map((a) => (
                  <button
                    key={a.id}
                    className={selected === a.id ? "selected" : ""}
                    onClick={() => select(a.id)}
                  >
                    <strong>
                      {a.name} #{a.slot}
                    </strong>
                    <small>
                      {a.mailbox.length} pending
                      {changed.has(a.id) ? " · changed" : ""}
                    </small>
                  </button>
                ))}
                {!entities.length && <p>No matching created entities.</p>}
                {entities.length > entityLimit && (
                  <button onClick={() => setEntityLimit(entityLimit + 100)}>
                    Show more entities
                  </button>
                )}
              </div>
            </>
          )}
        </aside>
        <section className="system">
          <div className="system-title">
            System{" "}
            <small>
              {frame?.actors.length ?? "…"} actors · highlighted entities
              changed
            </small>
            {frame && frame.actors.length > 200 && (
              <small>First 200 shown · select others in Entities</small>
            )}
          </div>
          {frame ? (
            <Graph
              frame={frame}
              changed={changed}
              selected={selected}
              select={select}
              positions={positions.current}
            />
          ) : (
            <div className="placeholder">Loading state…</div>
          )}
        </section>
        <Inspector
          frame={frame}
          previous={previous}
          actor={actor}
          meta={meta}
          endpoint={endpoint || index === last}
          select={select}
        />
      </main>
      <footer className="timeline">
        <div className="timeline-top">
          <span className={execution?.violation ? "violation" : ""}>
            {execution?.claim ?? "Loading execution…"}
          </span>
          {execution?.loop_start !== null &&
            execution?.loop_start !== undefined && (
              <button
                className="loop"
                onClick={() => navigate(execution.loop_start!)}
              >
                ↻ Loop to {execution.loop_start}
                {index === last ? " · closing state" : ""}
              </button>
            )}
          <span className="timeline-hint">Changes shown on the right</span>
        </div>
        <div className="scrubber">
          <button
            aria-label="First step"
            onClick={() => navigate(0)}
            disabled={!index}
          >
            ⇤
          </button>
          <button
            aria-label="Previous step"
            onClick={() => navigate(index - 1)}
            disabled={!index}
          >
            ←
          </button>
          <input
            aria-label="Snapshot"
            type="range"
            min={0}
            max={last}
            value={index}
            onChange={(e) => navigate(Number(e.target.value))}
          />
          <button
            aria-label="Next step"
            onClick={() => navigate(index + 1)}
            disabled={index === last}
          >
            →
          </button>
          <button
            aria-label="Last step"
            onClick={() => navigate(last)}
            disabled={index === last}
          >
            ⇥
          </button>
          <span className="step-number">
            {index} / {last}
          </span>
        </div>
      </footer>
    </div>
  );
}

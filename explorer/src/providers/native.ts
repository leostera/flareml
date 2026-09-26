import { Data, Effect, Schema } from "effect";
import { Metadata, Snapshot, Summaries, ExecutionTree } from "../trace/schema";

export class ExplorerError extends Data.TaggedError("ExplorerError")<{
  readonly message: string;
}> {}
export interface Provider {
  tree(): Effect.Effect<ExecutionTree, ExplorerError>;
  metadata(trace?: number): Effect.Effect<Metadata, ExplorerError>;
  snapshot(
    index: number,
    trace?: number,
  ): Effect.Effect<Snapshot, ExplorerError>;
  summaries(
    offset: number,
    filter: string,
  ): Effect.Effect<Summaries, ExplorerError>;
}
export function nativeProvider(): Provider {
  const token = window.location.hash.slice(1);
  // AbortSignal is tied to fiber interruption; React cleanup cancels stale work.
  const get = (path: string) =>
    Effect.tryPromise({
      try: async (signal) => {
        if (!/^[a-f0-9]{64}$/.test(token))
          throw new Error(
            "Missing session token. Open the complete URL printed by fml replay --ui --no-open, including its # token. For Vite development, configure FML_EXPLORER_ORIGIN and copy that token into the development URL fragment.",
          );
        const response = await fetch(path, {
          signal,
          headers: { Authorization: `Bearer ${token}` },
          cache: "no-store",
        });
        if (!response.ok) throw new Error(await response.text());
        if (
          response.headers
            .get("content-type")
            ?.split(";")[0]
            .trim()
            .toLowerCase() !== "application/json"
        ) {
          throw new Error(
            "The explorer API returned a web page instead of JSON. Open the URL from fml replay --ui --no-open, or configure the Vite development proxy with FML_EXPLORER_ORIGIN.",
          );
        }
        return (await response.json()) as unknown;
      },
      catch: (cause) =>
        new ExplorerError({
          message: cause instanceof Error ? cause.message : String(cause),
        }),
    });
  const decode = <A, I>(path: string, schema: Schema.Schema<A, I>) =>
    get(path).pipe(
      Effect.flatMap(Schema.decodeUnknown(schema)),
      Effect.mapError((cause) =>
        cause instanceof ExplorerError
          ? cause
          : new ExplorerError({ message: String(cause) }),
      ),
    );
  const cache = new Map<string, Snapshot>();
  return {
    tree: () => decode("/api/tree", ExecutionTree),
    metadata: (trace = 0) => decode(`/api/traces/${trace}/session`, Metadata),
    snapshot: (index, trace = 0) =>
      Effect.suspend(() => {
        const key = `${trace}:${index}`;
        const cached = cache.get(key);
        return cached
          ? Effect.succeed(cached)
          : decode(`/api/traces/${trace}/snapshot/${index}`, Snapshot).pipe(
              Effect.tap((snapshot) =>
                Effect.sync(() => {
                  cache.set(key, snapshot);
                  if (cache.size > 3) cache.delete(cache.keys().next().value!);
                }),
              ),
            );
      }),
    summaries: (offset, filter) =>
      decode(
        `/api/steps?offset=${offset}&filter=${encodeURIComponent(filter)}`,
        Summaries,
      ),
  };
}

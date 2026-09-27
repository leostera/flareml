import {specification,type Language} from './specs';
export const distributedTasks=['checkpoint-coordination','manifest-publication'] as const;
export type DistributedTask=typeof distributedTasks[number];
const common=`# Independent service processes: protocol v2
Create the complete project from scratch. No application code, build configuration, or tests are provided. Deliver your source, own tests, and DESIGN.md explaining safety, progress, assumptions, and linearization/coordination decisions.

Each process reads JSON lines and must synchronously emit and flush exactly ONE JSON action array (at most 64 actions) for EVERY input line, including init ([]). Never wait for future input while handling a line. Several operations may be outstanding. Diagnostics go to stderr. Processes have separate heaps. Use only the supplied actions for shared/durable application state: no files, sockets, clocks, randomness, external services, or other IPC. Testing and ordinary build files are allowed. Inputs are well-formed; output schemas are exact. Never emit unsolicited lines. Tokens are unique among outstanding operations within a process. Delayed results can interleave with all other inputs. A restart destroys volatile state. No HTTP layer is required. Do not encode a global application database in an unspecified channel. The evaluator runs real independent processes but controls transport/storage explicitly.
`;
const checkpoint=`# Distributed checkpoint coordination
Three replicas (node IDs 0,1,2) share an evaluator-provided consensus log, not shared application memory. You implement the coordination protocol, NOT the underlying consensus algorithm. There is one authoritative primary, with monotonically increasing epochs. Any replica can be promoted, including one holding a checkpoint lease. Each process has its own view, which may lag. Old-primary append attempts are fenced by the log service. The new primary is caught up with all currently committed log entries before its promotion event. Log application at other nodes can lag. A checkpointing node pauses log application but continues receiving view, time, completion, and result events. After restart the log can replay from entry 1. A restart of the current primary may retain its node/epoch authority; this is not a new promotion. Its volatile heap is empty and replay may be incomplete when other events arrive. There is no log-end/caught-up notification for this restart.

This is an explicitly simplified time model: a shared logical time, not arbitrary clock drift. Every input has now (integer). The runtime guarantees a checkpoint stops at its lease deadline even if the process is paused; completion/expiry notification can be delayed. A begun checkpoint needs two time units of work to complete, unless aborted or its deadline arrives first. If completion and deadline coincide, completion wins. Completion is durable; abort/expiry is not completion. A crash aborts current work. These facilities do NOT revoke other replicas' still-valid leases merely because the primary epoch changes.

Inputs:
- {kind:"init",node:0,nodes:[0,1,2],leader:0,epoch:1,now:0,completed:[]}: completed contains lease sequence numbers this node has already completed durably. Return [].
- {kind:"view",leader:1,epoch:2,now:0}: update local primary view. Views can arrive out of order; ignore older epochs. On promotion, any local checkpoint must be aborted in this same response before doing primary work.
- {kind:"tick",now:1}: time/progress opportunity.
- {kind:"apply",entry:{seq:1,epoch:1,holder:2,until:4},now:0}: next consensus-log entry. Applied entries are ordered per incarnation, but replay after restart is possible. A proposer may see its result before or after applying that entry.
- {kind:"result",token:"t1",ok:true,entry:{seq:1,epoch:1,holder:2,until:4},now:0}: log append result. On rejection ok=false and entry is null. A response can be delayed across a role change. Each issued result is eventually delivered unless its process incarnation crashes.
- {kind:"finished",seq:1,outcome:"completed"|"aborted",now:2}: checkpoint completion/expiry notification.

Actions:
- {kind:"propose",token:"t1",epoch:1,holder:2,until:4}: append a lease. At execution the service accepts only the CURRENT primary and epoch, a different holder, and now < until <= now+4. Otherwise it returns ok=false. Accepted entries get globally increasing sequence numbers. The service does NOT check overlapping leases or solve coordination for you.
- {kind:"start",seq:1}: begin local checkpoint using an already learned log entry for this node. Cannot start while already active, after deadline, after durable completion, or while primary. Starting an older replayed entry is not magically prevented by the runtime.
- {kind:"abort",seq:1}: stop the corresponding local active checkpoint, if still active. Aborting already-stopped work is an idempotent no-op.

Requirements: never have two checkpoints active simultaneously; the primary must never checkpoint. Do not revive obsolete authority after failover/replay. No checkpoint may outlive its deadline. Safety must hold during crashes, lag, and role changes. After stabilization, with all nodes alive, a stable primary, fair log/result delivery and ticks, both secondaries must complete useful checkpoints repeatedly (at least two each within 32 logical ticks). In this healthy suffix ticks advance one unit and pending log/results are promptly delivered, rather than delayed adversarially through lease expiry. Any of the three node IDs may be the stable primary; input examples do not privilege node 0. Refusing all work or only aborting does not pass. Judges include adverse prefixes followed by this healthy suffix. Execution has a generous 5000-event bound per scenario. There are no Byzantine processes or consensus inconsistencies. The consensus log contains ONLY the specified leases; no arbitrary global state or transaction API exists.
`;
const manifest=`# Concurrent immutable generation publication and cleanup
Two independent frontend processes publish and read artifact snapshots. Blob storage and the head catalog are separate services: no transaction spans them. Each artifact has immutable blobs indexed by a positive version and a single catalog head (null or one integer version). The head contains no payload or arbitrary bookkeeping. Each (artifact,version) has one immutable payload supplied by clients; retries use the same payload. Versions are externally assigned ordering keys, NOT necessarily arrival order. You must prevent the head from ever moving backwards. Multiple publications, readers and collectors can overlap.

Inputs:
- {kind:"init",node:0}: return [].
- {kind:"request",id:"r0",request:{op:"publish",artifact:"a",version:1,payload:"text"}}.
- request op read with artifact; or collect with artifact.
- {kind:"result",token:"t1",status:"ok",value:...}: storage completion. Mutations can instead return status="unknown", value=null: the operation MAY OR MAY NOT already have taken effect. Do not interpret an uncertain result as proof of no effect. Uncertainty is finite; later operations eventually succeed. Reads are reliable and linearizable. Completions can be reordered.

Actions:
- {kind:"head-read",artifact:"a",token:"t1"}: value is current head (integer or null).
- {kind:"head-cas",artifact:"a",expected:null,version:1,token:"t2"}: atomically replace head iff it equals expected. On status ok, value={swapped:boolean,current:integer|null}. On unknown, value=null. Service does not enforce monotonicity or referential integrity for you.
- {kind:"blob-put",artifact:"a",version:1,payload:"text",token:"t3"}: immutable idempotent put; status ok value=null. Different payload for an existing version is an error.
- {kind:"blob-read",artifact:"a",version:1,token:"t4"}: value is payload string or null.
- {kind:"blob-list",artifact:"a",token:"t5"}: value is sorted present versions, a snapshot which may become stale.
- {kind:"blob-delete",artifact:"a",version:1,token:"t6"}: idempotent delete; status ok value=null.
- {kind:"reply",id:"r0",result:...}.

Client results:
- publish -> {status:"published"} if this version was current at some point between invocation and reply; or {status:"superseded"} if a higher version was current in that interval. Never acknowledge a version that was not made current. No failure result: resolve finite uncertainty and contention by inspection/retry.
- read -> {status:"missing"} or {status:"ok",version:1,payload:"text"}, corresponding to a coherent head snapshot at some point between invocation and reply. A head pointing to an unavailable blob is NOT a valid missing result.
- collect -> {status:"ok"}. Reclaim obsolete blobs (versions strictly below current head). A quiescent collection must leave no obsolete blobs. Future staged versions may still be used and must not be reclaimed just because they are not the current head. Concurrent collection may conservatively leave blobs made obsolete after its snapshot; a later quiescent collection must reclaim them.

Always: every non-null head references an existing complete blob, including between operations and immediately after a crash; heads never regress; payloads match their versions. Reads must tolerate publication and cleanup interleavings. A caller can retry after losing its acknowledgement or frontend restart. In-flight storage effects may still happen after a frontend crashes, but their results cannot be delivered to a new incarnation. No shared volatile memory, metadata/blob transaction, locks supplied by another service, or extra storage keys are available. Max 32 client operations, strings <=128 characters, versions 1..32, storage messages <=64KiB. Each scenario must finish within 5000 scheduler events under finite faults and fair delivery.
`;
export function distributedSpecification(task:DistributedTask,language:Language){
 const languagePart=specification('payments',language).split('# Language / entrypoint\n')[1]!;
 return common+'\n'+(task==='checkpoint-coordination'?checkpoint:manifest)+'\n# Language / entrypoint\n'+languagePart;
}

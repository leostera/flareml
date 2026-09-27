# A genuine distributed recovery failure

The strengthened review reproduced overlapping checkpoint work in unchanged
candidate programs, not merely an invalid formal model or a serialization error.

Example: Rust baseline, original trial
`c5b50a16-223f-4cdc-ac97-d4bb64d8ee9d`.
Supplemental v2 trial: `b8636762-6d66-4954-a1a6-11c2ad61405b`,
case `recovery-backlog/seed-1`.

| Time | Event |
|---:|---|
| 0 | Primary 0 commits lease 1 to replica 1, deadline 4; replica 1 starts. |
| 4 | Lease 2 goes to replica 2, deadline 8; replica 2 starts. |
| 8 | Lease 3 goes to replica 1, deadline 12; replica 1 starts. |
| 8 | Primary 0 crashes/restarts, losing its volatile lease map. |
| 8 | Replay delivers only lease 1, whose deadline is already 4. |
| 8 | Primary treats that expired prefix as all outstanding authority and commits lease 4 to replica 2, deadline 12. |
| 8 | Replica 2 starts lease 4 while replica 1 is still performing lease 3. **Overlap.** |

The consensus service correctly accepts the current primary/epoch; it is not a
cross-node checkpoint lock. The missing information is the unreplayed tail of
an already durable log. Looking only at the maximum *learned* deadline is not
sufficient after process restart.

A conservative initial four-unit quarantine is sound under this benchmark's
shared-clock / maximum-four-unit-lease assumptions: every previously issued
lease must expire before a recovering primary can issue a new one. Other valid
solutions must respect the actual protocol; no log-end marker or extra storage
API can be assumed.

Some candidates reset holder rotation at restart. A three-entry prefix happened
to make their next holder match the active holder, masking the defect. The v3
review also tests a two-entry prefix, which exposes the same missing recovery
barrier in those programs. This is why passing the narrower v2 corpus was not
proof of correctness.

All supplemental versions and the original passing score are retained. No
candidate has been patched. See `judge-review-notes.md` for the post-hoc review
and its limitations.

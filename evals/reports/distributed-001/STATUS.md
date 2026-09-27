# Superseded diagnostic campaign

Stopped at the user's request to redo actual agent evaluations with corrected,
frozen judges rather than substitute artifact-regrading runs.

- 13 original generation trials completed/scored (including budget failures).
- Trial `b9503905-f87c-4ba7-a517-76dd979fbf1b` (manifest/Rust/TLA+) was explicitly
  interrupted, not scored as a finished generation. Its raw Pi stream and
  original temporary candidate workspace are retained; its usage is incomplete.
- The original scores, supplemental diagnostic reports and source snapshots
  remain available. They are not the new campaign's results.
- Replacement: **distributed-002**, all 24 cells generated from scratch again.

The frozen-artifact adapter and duplicate experimental checkpoint judge have
been retired from the live harness. Historical copies remain in the private
campaign archive. See `research/distributed-002.md` for the corrected design.

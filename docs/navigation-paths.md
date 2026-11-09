# Triangle-path selection

`ge-core::navigation::select_triangle_path` selects a mesh-local triangle chain
using the observed default search contract in SJBE52 revision 0. This is a runtime
building block, not actor movement or a list of waypoints. A separate
[waypoint builder](navigation-waypoints.md) consumes its returned chain.
Original execution and exact Wii numeric behavior have not been compared.

## Static evidence and rules

The original artifact identity is recorded in [triangle graph evidence](triangle-graphs.md).
Only semantic contracts and addresses are documented here. Original instructions,
analysis and game-derived route dumps stay under ignored `local/`; no original
code is linked into the independent Rust executable.

| Operation | Local original address |
| --- | --- |
| Explicit start/goal face search | `0x802e8100` |
| Default edge predicate | `0x802e73c0`, mesh policy field `+0x54`, callback slot `+0x08` |
| Default edge cost | `0x802e6cc0`, cost-policy callback slot `+0x08` |
| Default heuristic | `0x802e7060`, cost-policy callback slot `+0x0c` |
| Cost-policy object | Mesh field `+0x58`; default table `0x80591120` |
| Search-record allocation/release | `0x80030d80` / `0x80030ee0` |
| Default record-block configuration | `0x802e946c`: 256 records per block, 0x1c bytes per record |

The independent implementation preserves these observed decisions:

- Maintain ascending `g + h` order. Insert a new record after existing equal scores;
  do not break ties by face ID or replace FIFO ordering with a heap's incidental order.
- For an already open or closed face, reject only a strictly worse candidate `g`.
  An equal candidate replaces an open record or reopens a closed record. The comparisons
  at `0x802e8534` and `0x802e85c0` were decoded separately from ISA fields.
- Return when the goal record is removed from the open list, including start equal
  to goal. Follow parent records and include both start and goal in the returned chain.
- Visit neighbor slots in stored order and stop at the first boundary sentinel.
  The default edge predicate rejects the current record's immediate parent face.
- Use the stored edge-slot weight when present. Otherwise use center distance.
  The heuristic uses center distance independently of the stored weights.
- Use stored centers when present. Otherwise compute each coordinate as
  `(C + (A + B)) / 3` with separate f32 operations in the observed order.
- Return zero distance when squared distance is at most f32 epsilon. The original
  nonzero case uses Wii estimate/Newton arithmetic; the current Rust square root
  remains provisional, and can affect queue order near ties.

This contract is not a guarantee of a shortest-cost route. Heuristics can be
inconsistent with stored weights, and equal-cost replacement is observable.
Replacing it with generic Dijkstra or a conventional A* tie/reopen policy would
change the selected chain and policy-callback sequence.

## Parent records and host limits

Original parents refer to pooled record slots, not face IDs or immutable record
generations. The pool selects the newest nonfull block and its last free slot.
The Rust implementation models that logical slot reuse, including searches spanning
multiple 256-record blocks. A freed slot later reused for another record changes
what a parent reference reads. It does not embed original pointers or game code.
Releasing a record only updates free-slot metadata outside its payload; its old
contents remain readable until overwritten while that block is still allocated.

Host heap-address reuse after an entire original block is released is unknown.
Dangling parents and parent cycles report an explicit error. Self-edge expansion
can create conflicting original open/closed state; it is also an unsupported error
after the edge predicate accepts it. These cases are not silently repaired into
different paths. Static readers still preserve/report self links.

`max_expansions` is an independent host work guard, not a recovered game limit.
Exhaustion is an error, distinct from a completed search with no path. Inputs and
callback costs/estimates must be bounded, finite and nonnegative. Original malformed
data and exceptional-input behavior are not claimed. Cost/edge/heuristic callbacks
can be replaced through the pure API; actual actor-specific overrides remain pending.

## Commands and evidence

```powershell
.\target\release\goldeneye.exe nav-path '.\GoldenEye 007 (USA) (En,Fr).rvz' m_dam_gfx.edb 82000018 --start-triangle 0 --goal-triangle 100 --output local/triangle-path.json
.\target\release\goldeneye.exe scan-paths '.\GoldenEye 007 (USA) (En,Fr).rvz' --output local/path-audit.json
cargo xtask paths '.\GoldenEye 007 (USA) (En,Fr).rvz'
```

Example face IDs are illustrative, not an original actor request. `--index N`
disambiguates duplicate entity hashes. `--max-expansions N` changes the host guard
(CLI default 100,000). A null result path means the queue exhausted without finding
the goal. Output files preserve existing data and remain local when game-derived.

The audit picks up to eight start faces per root navmesh. Independent unweighted
reachability selects a farthest reachable goal for each start. It validates returned
endpoints, stored neighbor-prefix edges and the final search-record cost against an
independent f64 edge sum, with tolerance `1e-4 * (1 + abs(sum))`. It is not a shortest
path or original-runtime comparator. Detailed route IDs remain local.

The observed audit produced 542 routes across 68 navmeshes, 180,875 expansions,
20,252 record replacements and 34 closed-record reopenings. The longest chain had
254 faces and peak open-list size was 100. Maximum f64 cost-sum discrepancy was
`9.645e-5`; there were no query errors or same-face-only samples. These counts are
observations, not proof that arbitrary routes or actor policies are correct.

Authored tests cover FIFO ordering, equal-g open replacement/closed reopening,
boundary termination, default backtracking, weight/center fallbacks and epsilon
clamping, multi-block paths and slot reuse, invalid inputs and explicit work limits.
The source-bound `paths` receipt is separate from surface probes and contributes no
verified parity score. Remaining work includes Wii estimate precision, alternate
world binding, full actor policies, waypoint consumers,
movement/clearance and original-runtime comparison. The ordinary world-position
binding now runs with an explicit translation frame; see [world endpoints](navigation-world.md).

The next caller anchors are direct main-text branches at `0x80185d50` to the face
search, and `0x8007eb5c` / `0x80185ca0` to the alternate search. Cost/edge-policy
setter callers occur in those same wrapper areas, including `0x80185a7c` and
`0x80185a88`. The ordinary wrapper's lift/translation/downward query is established;
alternate goal predicates, actor context and instance binding remain pending.
The mesh-local alternate selector at `0x802e88f0` is now reconstructed with explicit
policy callbacks and an authored face-set CLI. See [predicate paths](navigation-goal-paths.md)
for callback timing, absent destination, squared point heuristic and scope limits.

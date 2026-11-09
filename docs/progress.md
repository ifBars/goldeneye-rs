# What the numbers mean

## Parity

`verified tracked criteria / all tracked parity criteria × 100`. Every criterion
has equal weight; the list is a **scope inventory**, not a percentage estimate of
total game effort or content. The initial scope is incomplete. A 100% tracked
score cannot establish full-game parity until the scope itself is audited against
all original content and behaviors. The dashboard calls this out explicitly.

States are `not_started`, `in_progress`, `implemented`. A future `verified` state
is deliberately rejected until a reference-comparison runner is implemented.
This prevents manual edits from manufacturing parity evidence. That runner must
capture edition/revision, input sequence, original reference identity, Rust build
hash, comparison output, tolerances, and failure cases. Screenshots alone cannot
prove physics/audio/timing parity.

## Engineering and coverage

Engineering gates show individually measured build, test, format, lint, coverage,
and local asset outcomes. They do not contribute to parity. Implementation
progress is the number of criteria marked implemented, separately labeled.

Coverage is LLVM's measured line and function coverage across production crates
and the app, excluding the tooling crate `xtask`. The dashboard names that scope;
tooling tests still run in `check`. Low coverage is visible, not disguised by
excluding difficult runtime paths. Coverage thresholds can be raised when justified;
the current CI line floor is 50%, a regression tripwire below the measured baseline
rather than a quality claim. It applies locally through `cargo xtask coverage` too.

## Freshness and compatibility

Receipts bind to source/configuration hashes, host OS/architecture, and timestamps.
Changes to hashed inputs mark receipts stale. A stale numeric coverage observation
is labeled stale and does not become a current pass. Future or malformed receipts
are errors, not passes. Raw logs are local; report summaries omit private image paths.

Compatibility is per edition/revision and operation. Passing asset intake for
SJBE52 rev0 RVZ does not mean the game is playable, other formats work, or another
region works. The acceptance list tracks gameplay and content separately.
The EDB/texture receipt is separate from disc/index intake. It records descriptor
parsing and decoded local image mips; unknown section bodies and external reference
graphs are not counted as validated. Successful decoding is not reference parity.
The same receipt resolves the grenade's table-five script descriptor with its
native reference/stride contract. This validates metadata only; it does not run
the script body or instantiate a live XGrenade component. Authored launch/arc
tests and staged CLI diagnostics remain separate from actual collision/body ticks.
It also audits root 0x601 static collision inputs. Enabled/disabled triangles in
included batches and excluded-batch declared triangles have separate denominators.
Raw flag histograms and f64 zero-area observations are input diagnostics, not
native contact outcomes, loaded instances, or unique world geometry. The ascent
box is checked with authored parabolic/chord diagnostics. Static contact
construction and the first-ascent request adapter now run against explicit
initialized world snapshots; live membership, skinning and original-runtime
precision remain unverified.
The same audit now decodes every root 0x607 triangle graph, preserving optional
arrays and tagged groups. It reports links, components, centroid/weight comparison
sample counts and errors separately. This does not validate unknown trailing body
fields, embedded non-root entities, or original navigation/physics behavior.
The native render receipt proves the explicit GPU fixture and one real Dam
map inspection ran on the named adapter/backend. Strict placement decode and
incomplete inspection are reported separately. The compatibility cell remains
`partial` for a fresh passing inspection receipt, including when all placements
decode. The current shader, visibility and reference comparison are incomplete;
successful decoding or a PNG cannot turn that into full compatibility. It does not establish
playability, complete level rendering, frame-time performance, other GPU support,
or reference visual parity. Placements and shader/state omissions are reported.

The native-window receipt is a separate gate. It records bounded hidden surface
presentation, a resize, one scene upload, and exact RGBA agreement with the shared
offscreen path. It proves neither visible-window interaction nor original-game
parity. The compatibility table marks its scene support `partial` independently
of placement decode counts. CPU input/camera invariants and the GPU regression run separately.

Navigation surface queries have a separate sampled-smoke receipt. It records root
navmeshes probed, separate storage-first/nearest/selected-face/connected-face query counts,
hit/miss results, group handling and geometric
error checks. Connected samples have depth one and separately report face tests,
neighbor hits, policy checks and skipped samples. They do not establish deep routes
or actor-specific policy behavior. The audit is explicitly `reference_compared: false`: matching ordinary
geometry checks cannot establish Wii estimate precision, original runtime behavior,
world transforms, pathfinding or gameplay parity. It does not contribute to the
verified parity percentage.

Triangle-path selection has its own sampled `paths` receipt. Farthest-reachable
goals exercise chains beyond one-hop queries. Endpoints, neighbor-prefix edges and
an independent f64 edge-cost sum are checked; expansion/replacement/reopen counts
remain separate from path length. This is not an optimal-cost or original-runtime
comparator and does not verify actor movement. It contributes no parity numerator.
The same receipt separately counts world requests, grounded pairs, paths, endpoint
misses and completed search misses. Those samples use an authored translation;
endpoint geometry, missing-hit candidates, graph reachability and route costs are
checked independently. A validated endpoint miss is not counted as a path found,
and does not establish original placement/actor-request compatibility.
Waypoint samples separately count constructed paths, seeds, retained points,
shortcut tests, fan steps and flag rejections. Structural/frame invariants and
independent f64 XZ portal checks are distinct from authored fan tests, actor
movement and original-runtime clearance. Point reduction is not a parity score.
Read-only route consumers separately count target/length checks, arrival samples,
and corridor probes. The f64 corridor oracle reports validated and near-boundary
ambiguous samples separately; ambiguous samples do not become comparator passes.
No route-consumer sample establishes point advancement, actor movement, original
fourth-component/FP state or reference parity.
Actor dispatch samples separately count calls, acquisitions, acquisition misses,
stored-AABB exclusions, deferred queries and route successes. A processed call
does not mean a path exists. Each sampled graph is an authored single-candidate
context; these counts do not establish original manager membership or scheduling.
The resource receipt also counts static map directories, placement records,
registration readiness/unavailability, unresolved source references and nonzero
translations. Shared mesh counts sum each map's distinct entity slots; they are
not a count of original simultaneous residency. Static bindings do not prove
loaded-map precedence, dynamic state, original actor inputs or gameplay parity.

Initial boundary-cache evidence separately counts caches, pairs, topology faces,
sampled first-hit queries, f64 agreements and numerically ambiguous queries. It
preserves raw input coordinates and disjoint-collinear acceptance. These are
diagnostics, not physical clearance or original-runtime comparisons; ambiguous
samples do not become passes or contribute to verified parity.
Actor boundary bands separately count authored eligible samples, f64 diagnostic
agreements, numeric ambiguity and reconstructed acceptance outcomes. Acceptance
is not a reference pass or parity score. Mode/range/height/binding ordering and
target-record selection have authored tests; live actor actions remain unverified.
Persistent route sequences separately count wrapper calls, successful replacements,
retained endpoint misses, independently checked outside positions and duplicate
final-pair rebuilds. They test state transitions over authored positions, not actor
movement, waypoint popping or original-runtime parity.
Predicate-path diagnostics count authored accepted-face queries, queue-removal goal
tests, replacements/reopenings and independently checked squared heuristics. They
share the ordinary record pool, but use explicit point/goal inputs. These counts
do not establish original actor goals or shortest paths.
Optional actor-edge diagnostics separately count destination flag, stored-normal
and direction rejections, f64 agreements, numeric ambiguity and constrained search
outcomes. Stored vector absence is preserved. Authored points/goals and independent
reachability checks do not establish actual actor inputs, Wii normalization or
reference parity. Expected constrained misses are successful diagnostic outcomes.
Geometric actor-goal diagnostics separately count adjacent-face samples, f64
agreements, ambiguous geometry and unsupported walks. Full searches distinguish
found paths, completed misses, unsupported errors and expansion-budget exhaustion.
Unsupported/budget cases contribute no comparator pass and never become misses.
The goal callback is observed; origin/target/start-face inputs remain authored.
Actor caller samples separately count storage-first target hits, original-target
height rejection, connected active-face hits/misses and prepared inputs. Manager
flag transitions have their own count. Independent f64 binding and path checks
remain separate from unsupported subsequent goal walks and search-budget errors.
World snapshots/frames are authored; no live action gate or state mutation is proved.
The separate action audit now counts aligned/opposed authored calls, selected-record
updates, retained no-ops and explicit host errors. Independent f64 motion
approximations and angular checks have separate denominators; near-limit angles
remain ambiguous. A stored motion vector does not prove integration, collision,
live record scheduling or original-runtime parity.
Alternate world-route counts distinguish initial authored requests from total
wrapper calls, start misses, predicate misses, waypoint paths, later retention
checks and recoveries. Raw centers and returned endpoints are distinct; round-trip
change counts measure arithmetic observations, not parity or physical correctness.

`static_mesh_contacts` is a separate authored snapshot audit. It samples at most
eight stored zone centers per map, with radius eight, an identity query frame,
an identity-axis box centered on the sample with half extents `(1,1,1)`, filter
slot/flags zero, and record/contact capacities 4,096. Its counters describe
collection, resident-cache preparation/reuse, coarse/batch/triangle rejection,
primitive calls, appended contacts and capacity outcomes. Finite-value, normal
and provenance checks validate the returned data's stated invariants; they do
not provide independent contact truth. An empty or saturated sample, successful
process, or internally consistent contact does not establish live clearance or
original-reference parity. This audit does not change the roadmap scope gates.

`world_box_requests` samples the same bounded zone-center positions with request
flags zero and two, radius eight, an identity query frame, one unit-half-extent
box, filter slot/flags zero and record/contact capacities 4,096. The single-zone
result must match direct selected-zone records and contacts. Expanded records
must match ordered single-zone cohorts under one global record cap, including
duplicate zones; contacts are not concatenated across zones because inverse-cache
keys can couple consecutive records. Per-zone ranges must cover the combined list
without a reset. Finite/identity checks and source triangle/material provenance
validate contact metadata; BSP, portal, capacity and preparation counters retain
their own denominators. These are composition checks, not original-game truth.

The separate `grenade-ascent-box` command uses source filter slot 1/flags 3,
portal flag 2 and capacities 64/512. It covers the first ascent and derives
`blocked` from contact count, without accepting later line/actor clearance or
advancing full action state. The illustrative world list remains authored.
Successful requests do not establish live membership, ready-to-launch state,
body activation or parity. See [world contact request contracts](world-contact-requests.md).

`collision-world-line` is a separate authored snapshot diagnostic. Its BSP
node/leaf/zone counts, geometry/coarse gates, resident cache and plane preparation,
triangle tests, hit/miss state and provenance describe operations performed on
the explicit request. The sixteenth-zone `capacity_reached` marker is a source
stop condition, not proof that another zone was omitted; stack/work exhaustion
is an error. Equal-hit replacement and early-stop results have different meanings,
and an early-stop result deliberately has no world-normal conversion. No line
hit, miss, counter or authored composition test contributes to verified parity.
There is no independent original-contact truth implied by these CLI outputs.
See [world line query contracts](world-line-queries.md). Actor integration and
full grenade state acceptance remain separate from a static world result.
`grenade-world-line` uses the typed cached-apex constructor, pre-lift bounds,
lifted displacement and source filter settings. Its `blocked` field covers world
geometry only. A false value does not include the actor-on-miss phase, accept the
full action state or activate a physics body.

History contains actual generated observations only. A first report has one point;
the tool never invents a trend. Roadmap changes affect the denominator and are
included in the observation identity. `latest.json` is suitable for other tools;
`index.html` embeds it so `file://` viewing needs no fetch/server.

The history chart plots verified tracked parity, declared implemented scope and
production line coverage as separate labeled series. Coverage points require a
passing measured receipt at that observation; missing, failed or stale evidence
breaks the line instead of inventing continuity. Historic successful observations
remain observations of their recorded source, not a claim about the current build.
`tools/dashboard` tests missing evidence, freshness labels, filters, safe text,
single-point history and coverage gaps without private game data.

# Actor target caller binding

`nav-actor-target` reconstructs the surface-binding and policy-search stages of
the target action at `0x8007e6e0`. It consumes explicit world snapshots and separate
owner/instance translations. It runs after the caller's external action predicate;
the separate [action stage](actor-target-action.md) now implements that angular
predicate and selected-record update over an authored snapshot. Live record loading,
scheduling and movement remain open. The ordinary/world route wrapper has its own contract.

## Static evidence

The local SJBE52 revision 0 artifact identity is recorded in
[triangle graphs](triangle-graphs.md). Original instructions remain under ignored
`local/`; no original executable code is linked or packaged.

| Contract | Original location |
| --- | --- |
| Separate external predicate | `0x8007e700`–`0x8007e7b4`, calls `0x80059480`; outside this implemented stage |
| Owner-local target | World target at caller `+0xa0`, minus actor's owner origin at `actor[+4]+0xf8`; `0x8007e7b8`–`0x8007e82c` |
| Initial mesh | First cached binding at actor `+0x5f8`, mesh at binding `+0x1c`; `0x8007e83c` |
| Lift/direction | Global vector `0x805c9718` is added, then negated for the ray; `0x8007e838`–`0x8007e8e4` |
| Vector initializer | `0x80008e20`, initializer-table reference `0x8053ac88`; stores at `0x80008ee8/0x80008eec` establish `(0,1,0,0)` from constants zero/one |
| First surface call | Navmesh slot `+0x4c`, storage-first, group -1, strict `0<t<1.25`; `0x8007e8e8`–`0x8007e900` |
| Height gate | `0x8007e904`–`0x8007e914`: reject `abs(original owner-local target Y)>=1`, after a successful ray |
| Binding reload | `0x8007e918`–`0x8007e920`, may now be absent; retain the first mesh pointer |
| Active segment | Active record selected by caller `+0xd4`, minus the reloaded binding origin, then local Y+1; displacement `(0,-2,0)`, previous=-1, depth five; `0x8007e924`–`0x8007ea1c` |
| Manager policy installation | `0x8007e9b4`–`0x8007e9c8`, callback object `0x805d4b54` |
| Manager policy identity | Initializer `0x80188570`, initializer-table reference `0x8053ae74`; object vtable `0x80579d68`, callback `0x80186210` |
| Goal inputs | Unraised active position minus instance origin at context `+0x0c`; target minus instance origin at `+0x1c`; connected hit face at `+0x3c`; `0x8007ea20`–`0x8007eb3c` |
| Policy search | Installs actor edge/cost/goal interfaces, calls alternate selector, restores manager policies; `0x8007eb40`–`0x8007eb80` |

The first query returns a face index. The later height check uses a retained copy
of the original local target, not a returned surface position or normal. Reusing
the ordinary nearest-hit world endpoint binding would change selection and limits.

The second binding supplies the translation for the active segment and both policy
points. It can differ from the owner translation. Its frame lookup is lazy and may
fail, while its mesh pointer does not replace the captured first mesh. On success,
the policy origin remains unraised and both inputs remain unsnapped. Target-minus-
instance subtraction occurs only after the connected lookup succeeds.

## Manager traversal policy

This policy is different from both default immediate-backtracking rejection and
the actor target-direction filter:

1. Reject immediate backtracking before reading flags.
2. Reject entering a destination with bit `0x02` when the source lacks `0x02`.
3. Reject a source carrying `0x10` entering a destination carrying `0x08`.
4. Otherwise allow, preserving all unrelated bits; absent flag arrays mean zero.

The connected lookup retains current-face-first testing, depth-first slot order,
first-sentinel termination and inclusive segment endpoints. A current-face hit
does not consult this edge policy. The fallible-policy API propagates a checked
callback error only when the lookup actually reaches that edge.

## Diagnostic and limits

```powershell
.\target\release\goldeneye.exe nav-actor-target '<your-disc>' '<resource.edb>' --index 0 --owner-translation 0 0 0 --instance-translation 0 0 0 --target 10 0 10 --active-position 0 0 0 --output local/actor-target.json
```

Values above illustrate input shape; they are not a known valid game request.
`--max-face-tests`, `--max-expansions` and `--max-goal-steps` are separate positive
host guards. Exhaustion and unsupported goal crossings are errors, never successful
misses. Outputs preserve existing files. Detailed game-derived records stay local.

Output distinguishes no initial face, rejected original target height, lost later
binding, no connected active face and prepared policy inputs. A prepared binding
can still produce a completed search miss or an error. No ordinary wrapper status,
retained waypoint route, original action return value or movement update is inferred.

Authored tests cover distinct frames, storage-first versus nearest, strict ray
limits, both height equalities, binding loss, lazy unused values, callback errors,
manager flag transitions and independent host budgets. Corpus diagnostics use two
authored requests per mesh: a far topological target, and the lowest-absolute-Y
stored center with a neighboring active face. Scalar f64 plane/barycentric and
depth-first checks classify numerical ambiguity separately. These are host
diagnostics, not original actor snapshots, action selection or runtime parity.

The initial corpus has 136 authored snapshots: all initial first-hit choices and
54 connected results agree with independent scalar checks. Original target height
rejects 82 requests; 18 lack a connected active face and 36 prepare policy inputs.
Those prepared searches produce 12 paths, one completed miss and 23 explicit
unsupported goal walks. No unsupported walk is counted as a miss or parity pass.
All 147,062 manager flag transitions agree with independent bit conditions, with
146,237 allowed. These counts describe this sampler and source revision only.

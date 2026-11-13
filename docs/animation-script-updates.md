# Retained script updates

The fresh New Game path now returns from its TP setter and TP dispatch. It stops
before FP dispatch, with player creation and the gameplay camera still pending.
This is a native loading checkpoint, not a playable mission or a parity result.

## Setter and dispatch order

The setter uses the earlier FP mode-availability result to choose its history
branch. The current native result is available, so this TP call returns without
writing player mode history. A tracked nonzero selection instead writes the
requested generic and mapped modes before examining the new primary head. A
changed head identity or script reference clears secondary history. Zero returns
skip those stores and head reads.

TP dispatch retains its slot-two position query from before control capture.
That actual list was empty, so the captured value is positive zero. After the
setter returns, three ordered helpers inspect slots zero, two and three. The
snapshot control in slot zero does not have the script subtype. The other two
heads are the inserted scripts, and their blend coordinates receive the actual
player inputs. Each subsequent update copies the captured phase before entering
the script helper with initflag1.

## Re-entry through the same owners

`ge-core/animation/script_child/update` records each invocation separately from
the first factory initializer. It checks the retained script wrapper's generation
and body, queries the existing clip rate, selects the reached play command and
prepares its position. A matching selected offset skips first-reference, clip
setup, index and play-flag reads. It reuses the existing allocation and caches.
A changed offset remains pending before clip replacement's release/setup work.

The clip-rate helper visits normal clip thresholds in source order and resolves
only the selected animation bodies. It preserves independent cache writes,
missing bodies, unavailable providers, blend fractions and both FPS reads. The
retained unsigned baseline is a separate input from float values. The native
first two queries each produce 0.5 from 30 FPS and baseline60. The host frame's
scaled step remains an actual clock input; original timing is unverified.

Initflag1 skips the event/callback branch. A returned update does not sample a
pose, publish a player or create a camera. Required timing curves, root motion,
unavailable file owners and changed clip storage remain explicit dependencies.

## Failure state and evidence

Each successful prefix remains owned when the next provider fails. Pending or
failed invocations cannot be replayed as fresh initialization or acknowledged as
returned. Diagnostic history retains the latest eight invocation traces and a
total counter, so its storage limit does not end the simulation after a fixed
number of updates. The original factory traces remain separate.

The dashboard reports setter return, history writes, scalar visits, script
invocations, rate returns, clip reuse and position returns. The initial-action
callback and gameplay readiness remain separate. Authored tests cover skipped
reads, retained backing, ordered cache failures, pending providers and replay
rejection; native probes check the reached game-data path. Neither establishes
reference runtime or numerical parity.

# Post-trigger resource preparation

After trigger construction and navigation initialization, the native mission worker
now performs the next selected-definition hook. The pure operation lives under
`ge-core/src/mission/post_trigger`; actual supplied-data ownership belongs under
`game/world/post_trigger_resources`. This component does not complete world
initialization or create a player.

The admitted New Game descriptor makes the source predicate false. The selected
level row then supplies one definition reference to the **same mutable registry**
already used by Actor12 constructors. Tutorial's ten constructor selections leave
group8's cursor at 2; this hook consumes the eleventh selection and leaves it at 3.
It does not draw another random value or reset the registry. A retained operation
holds that selection and its two requests, so inspecting or retrying the plan
does not consume another selection. Unbound default/special definitions remain
explicit failures.

The selected definition supplies two ordered targets in the same file. The
worker uses the existing bounded mission loader, retains the full immutable EDB
once, and resolves each target through the native coded/hash reference rules.
The checked `target_section` reader supports the reached skin and texture
directories. Its private-constructor view borrows the owning EDB and supplies
only its validated section span; caller-provided metadata cannot authorize a
different read. Raw section indices retain their high-bit masking rule.

The exact section key `08000000` is the source base case and requires no streamed
span. Other supported section keys retain checked payload bytes. Repeated targets
can share a file and section without removing either source call. Empty spans
remain real validated spans. Covered section bytes count the union of reached
ranges within each file, so overlapping sections are not counted twice.

The source caller ignores both returned wrapper values. Accordingly, a missing
target or failed first read still allows the second request and the proven sparse
tail reset. The component separately requires actual successful target/residency
checks before reporting completion. Failed outcomes retain their consumed plan
and private errors; they do not become an empty successful load. The reset owns
only four zero words, a sentinel word, a positive-zero scalar and a cleared byte;
unwritten manager fields are not invented.

This is an explicit synchronous supplied-data residency adaptation. It does not
implement or verify the original section relocation, streaming job queues,
allocator pointers, earlier manager state or original timing. A source metadata
state or non-null base wrapper alone never proves section residency. The local
Tutorial check makes two requests, retains one 5,524,960-byte file and covers one
647,392-byte section. Those bytes and all three receipt fingerprints are checked
against an independent raw-data evaluator; original payload arrays remain private.

Remaining world hooks still need their actual engine-time inputs, buffer cleanup,
optional binding releases and callback ownership before the source phase18
catalog rebuild and identity-bound readiness event. Player, camera and gameplay
remain incomplete.

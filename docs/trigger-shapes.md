# Checked trigger shape construction

Common shape binding retains an owner-checked selector and type. Geometry is
decoded only when a supported constructor reaches a geometry consumer. The
type0 view borrows the map's EDB, resolves its own reference/selector, checks the
full 48-byte descriptor span and preserves raw scalar/vector bits. The map used
for a spatial zone query can differ from the owning map; it cannot supply this
descriptor implicitly. Neither view nor original arrays implement serialization.

`ge-core/mission/triggers/shape71` owns the initial subtype71 construction. Its
source constructor initializes only a tag. Present type0 descriptors build an
owned internal type2 payload, then apply the serialized trigger placement. The
placement uses the audited Rx/Ry/Rz source order and polynomial arithmetic;
the existing asset placement builder has a different order and scale operation.
Quaternion conversion reuses the separately audited pure helper. Axis products
retain all four terms, including zero products and W handling; no normalization
or identity substitution is applied.

Uninitialized, materialized, placement-built and transformed stages remain
distinct. Failure after materialization retains that payload while withholding
membership. The absent-property fallback writes a sparse type0 payload; its
unwritten storage has no invented zero fields. A selector of zero is a present
descriptor, rather than the fallback branch.

The app binds geometry from the same EDB and retained common binding on the CPU
worker. Original selector order and ordinal slots remain intact, including staged
objects. Public evidence reports selector and materialization counts plus a
private-state digest, without source geometry arrays. The current Tutorial owns
nine selector bindings and four transformed type2 shapes. Original hardware
precision, later collision queries/events, physics and world readiness remain
unverified or incomplete.

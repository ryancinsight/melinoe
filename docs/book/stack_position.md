# 8. Position in the Stack

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - Atlas layer: eunomia → melinoe (parallel to themis) → mnemosyne / moirai
  - mnemosyne uses melinoe for branded heap access: BrandedBox/BrandedVec
    give the allocator compile-time proof that heap cells are exclusively or
    shared-ly accessed, enabling lock-free pool patterns
  - moirai uses melinoe for task-local state: ThreadLocalToken gates ambient
    task state so re-entrant task execution cannot alias its own state
  - The halo module: melinoe's internal self-use for its own capabilities
    (the "halo" is melinoe applied to itself to bootstrap the token proof)
  - themis integration: ConstNumaPinnedCell / ConstNumaPinnedSlice are branded
    with a placement proof so a NUMA-pinned allocation cannot be moved to a
    different NUMA node without the compiler catching it
  - What melinoe does NOT own: allocation (mnemosyne), execution (moirai),
    NUMA placement (themis), physical quantities (aequitas)
-->

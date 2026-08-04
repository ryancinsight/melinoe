# 3. Multi-Token Composition

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - Nested brand_scope: fresh brand per closure; two independent exclusion
    domains live simultaneously, proven disjoint at compile time
  - WriterShard: splits one brand's write region into N disjoint sub-regions;
    each shard covers a non-overlapping slice of the cell array
  - partition_for_each_with: drives concurrent writes via WriterShard without
    a runtime lock; PartitionPlan::chunk_size / by_count / hardware_parallelism
  - SyncRegionToken: moves a brand's write capability across thread boundaries
    when the whole region crosses a thread boundary (e.g. spawned task)
  - The key invariant: no two shards ever overlap; the type system rejects
    attempts to alias across shard boundaries
-->

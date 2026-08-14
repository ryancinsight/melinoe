//! Value-semantic tests for the indexed disjoint-shard accessor
//! [`WriterShard::par_chunks`] / [`ParChunks`], the random-access counterpart to
//! the sequential [`ShardChunks`](melinoe::region::ShardChunks) iterator used by
//! work-stealing consumers (e.g. `moirai-parallel`).

use melinoe::region::{ParChunks, WriterShard};
use melinoe::{brand_scope, MelinoeCell};

/// `len()` reports the exact partition count `ceil(n / chunk)` for representative
/// (n, chunk) pairs, and `0` for an empty region.
#[test]
fn par_chunks_len_is_exact_ceiling() {
    brand_scope(|_token| {
        // (n, chunk, expected partitions)
        let cases: &[(usize, usize, usize)] = &[
            (0, 4, 0),
            (1, 4, 1),
            (4, 4, 1),
            (5, 4, 2),
            (10, 3, 4), // 3,3,3,1
            (10, 1, 10),
            (7, 10, 1),
        ];
        for &(n, chunk, expected) in cases {
            let mut cells: Vec<MelinoeCell<'_, u8>> = (0..n).map(|_| MelinoeCell::new(0)).collect();
            let par = WriterShard::new(&mut cells).par_chunks(chunk);
            assert_eq!(par.len(), expected, "n={n} chunk={chunk}");
            assert_eq!(par.is_empty(), expected == 0, "n={n} chunk={chunk}");
        }
    });
}

/// `chunk_size` is clamped to at least one, matching the sequential `chunks`.
#[test]
fn par_chunks_zero_chunk_is_clamped_to_one() {
    brand_scope(|_token| {
        let mut cells: Vec<MelinoeCell<'_, u8>> = (0..5).map(|_| MelinoeCell::new(0)).collect();
        let par = WriterShard::new(&mut cells).par_chunks(0);
        // Clamped to chunk size 1 → one partition per cell.
        assert_eq!(par.len(), 5);
    });
}

/// Partition property: fetching every index in `0..len()` yields shards whose
/// lengths sum to `n` and whose global offsets tile `0..n` exactly once (no gap,
/// no overlap). Each index is requested exactly once, satisfying the accessor
/// contract.
#[test]
fn par_chunks_partitions_cover_region_exactly_once() {
    brand_scope(|token| {
        const N: usize = 23;
        const CHUNK: usize = 4;
        let mut cells: Vec<MelinoeCell<'_, usize>> = (0..N).map(|_| MelinoeCell::new(0)).collect();

        let par = WriterShard::new(&mut cells).par_chunks(CHUNK);
        let num = par.len();
        assert_eq!(num, N.div_ceil(CHUNK)); // 6 partitions: 4,4,4,4,4,3

        let mut total_len = 0usize;
        let mut expected_start = 0usize;
        for index in 0..num {
            // SAFETY: `index` ranges over `0..par.len()` exactly once, and each
            // returned shard is dropped before the next `get` (sequential loop),
            // so no two shards alias — the accessor contract holds.
            let mut shard = unsafe { par.get_unchecked_chunk(index) };
            // The shard sits at global offset `index * CHUNK`; write that offset
            // into every one of its cells so we can prove tiling from the region.
            let start = index * CHUNK;
            assert_eq!(
                start, expected_start,
                "partitions are gap-free and in order"
            );
            for (j, slot) in shard.iter_mut().enumerate() {
                *slot = start + j;
            }
            total_len += shard.len();
            expected_start += shard.len();
        }
        assert_eq!(total_len, N, "partitions cover every cell");

        // Every cell now holds its own global index → disjoint, complete tiling.
        let snap = token.share();
        for (k, c) in cells.iter().enumerate() {
            assert_eq!(*c.borrow(snap), k);
        }
    });
}

/// Disjoint mutation through two *different* indices held live simultaneously
/// does not alias: distinct values written via `get(i)` and `get(j)` both land.
#[test]
fn par_chunks_disjoint_indices_do_not_alias() {
    brand_scope(|token| {
        // Six cells, chunk 2 → three partitions [0,1] [2,3] [4,5].
        let mut cells: [MelinoeCell<'_, i64>; 6] = core::array::from_fn(|_| MelinoeCell::new(-1));
        let par = WriterShard::new(&mut cells).par_chunks(2);
        assert_eq!(par.len(), 3);

        // Hold partitions 0 and 2 live at the same time and write distinct
        // sentinels through each. If they aliased, one write would clobber the
        // other; the disjoint-range guarantee makes both independent. The inner
        // scope ends the shards' (and `par`'s) exclusive borrow of `cells` before
        // the token read below.
        {
            // SAFETY: indices 0 and 2 are distinct and each requested exactly
            // once, so their `[0,2)` and `[4,6)` ranges do not overlap.
            let mut a = unsafe { par.get_unchecked_chunk(0) };
            let mut b = unsafe { par.get_unchecked_chunk(2) };
            for slot in &mut a {
                *slot = 7;
            }
            for slot in &mut b {
                *slot = 9;
            }
        }

        let snap = token.share();
        let seen: [i64; 6] = core::array::from_fn(|k| *cells[k].borrow(snap));
        // Partition 1 ([2,3]) untouched (-1); 0 → 7, 2 → 9.
        assert_eq!(seen, [7, 7, -1, -1, 9, 9]);
    });
}

/// A single partition covering a whole small region reads and writes as the full
/// slice, and `get_unchecked_chunk` returns a shard whose `as_slice` matches.
#[test]
fn par_chunks_single_partition_is_whole_region() {
    brand_scope(|token| {
        let mut cells: [MelinoeCell<'_, u32>; 3] =
            core::array::from_fn(|i| MelinoeCell::new(i as u32));
        let par = WriterShard::new(&mut cells).par_chunks(8); // chunk > len → 1 partition
        assert_eq!(par.len(), 1);

        // The inner scope ends the shard's borrow of `cells` before the token
        // read below.
        {
            // SAFETY: index 0 is the only partition and is requested once.
            let mut shard = unsafe { par.get_unchecked_chunk(0) };
            assert_eq!(shard.as_slice(), &[0, 1, 2]);
            for slot in &mut shard {
                *slot += 100;
            }
        }

        let snap = token.share();
        let seen: [u32; 3] = core::array::from_fn(|k| *cells[k].borrow(snap));
        assert_eq!(seen, [100, 101, 102]);
    });
}

/// `ParChunks` is `Send + Sync` (when `T: Send`), so a driver can move the view
/// and hand shards across threads — the property a work-stealing pool relies on.
#[test]
fn par_chunks_is_send_sync() {
    fn assert_send_sync<X: Send + Sync>() {}
    assert_send_sync::<ParChunks<'static, 'static, u64>>();
}

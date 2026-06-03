//! Compile-time, zero-cost proofs of the crate's layout guarantees.
//!
//! These `const` blocks are evaluated during compilation and emit no code. A
//! violation (e.g. a token gaining a non-ZST field, or a guard losing its
//! transparent layout) is a hard compile error, lifting the crate's "zero-cost
//! / zero-sized" claims from prose to the type-level evidence tier.

use core::mem::{align_of, size_of};

use crate::region::WriterShard;
use crate::sync::{SyncRegionToken, ThreadLocalToken};
use crate::token::{ExclusiveToken, SharedReadToken};
use crate::{MelinoeCell, MelinoeCell2, MelinoeMut, MelinoeRef};

const _: () = {
    // ── Every capability token is zero-sized: carrying a permit through an
    //    API costs no memory and no register after monomorphization. ──
    assert!(size_of::<ExclusiveToken<'static>>() == 0);
    assert!(size_of::<SharedReadToken<'static, 'static>>() == 0);
    assert!(size_of::<ThreadLocalToken<'static>>() == 0);
    assert!(size_of::<SyncRegionToken<'static>>() == 0);

    // ── A branded cell adds nothing beyond its payload: no borrow flag, no
    //    discriminant, no padding (contrast `RefCell`, which carries a counter). ──
    assert!(size_of::<MelinoeCell<'static, u64>>() == size_of::<u64>());
    assert!(align_of::<MelinoeCell<'static, u64>>() == align_of::<u64>());
    assert!(size_of::<MelinoeCell<'static, [u8; 37]>>() == 37);
    assert!(size_of::<MelinoeCell<'static, ()>>() == 0);

    // ── A two-brand cell is likewise exactly its payload (both markers ZST). ──
    assert!(size_of::<MelinoeCell2<'static, 'static, u64>>() == size_of::<u64>());
    assert!(align_of::<MelinoeCell2<'static, 'static, u64>>() == align_of::<u64>());

    // ── Borrow guards are ABI-identical to the bare reference they wrap, and
    //    inherit its null-pointer niche (so `Option<Guard>` stays pointer-sized). ──
    assert!(size_of::<MelinoeRef<'static, 'static, u64>>() == size_of::<&u64>());
    assert!(size_of::<MelinoeMut<'static, 'static, u64>>() == size_of::<&mut u64>());
    assert!(size_of::<Option<MelinoeRef<'static, 'static, u64>>>() == size_of::<&u64>());
    assert!(size_of::<Option<MelinoeMut<'static, 'static, u64>>>() == size_of::<&mut u64>());

    // ── A writer shard is exactly its underlying `&mut [MelinoeCell]` slice
    //    reference (a fat pointer): the partition capability adds no footprint. ──
    assert!(
        size_of::<WriterShard<'static, 'static, u64>>()
            == size_of::<&mut [MelinoeCell<'static, u64>]>()
    );
};

//! Compile-time, zero-cost proofs of the crate's layout guarantees.
//!
//! These `const` blocks are evaluated during compilation and emit no code. A
//! violation (e.g. a token gaining a non-ZST field, or a guard losing its
//! transparent layout) is a hard compile error, lifting the crate's "zero-cost
//! / zero-sized" claims from prose to the type-level evidence tier.

use core::mem::{align_of, size_of};

use core::sync::atomic::AtomicUsize;

use crate::region::WriterShard;
use crate::sync::{SyncRegionToken, ThreadLocalToken};
use crate::token::{ExclusiveToken, SharedReadToken};
use crate::{AcqRel, BrandedAtomic, MelinoeCell, MelinoeMut, MelinoeRef, Relaxed, SeqCst};
#[cfg(feature = "alloc")]
use crate::{Borrowed, Retained};

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

    // ── Borrow guards are ABI-identical to the bare reference they wrap, and
    //    inherit its null-pointer niche (so `Option<Guard>` stays pointer-sized). ──
    assert!(size_of::<MelinoeRef<'static, 'static, u64>>() == size_of::<&u64>());
    assert!(size_of::<MelinoeMut<'static, 'static, u64>>() == size_of::<&mut u64>());
    assert!(size_of::<Option<MelinoeRef<'static, 'static, u64>>>() == size_of::<&u64>());
    assert!(size_of::<Option<MelinoeMut<'static, 'static, u64>>>() == size_of::<&mut u64>());

    // ── Projection (`map`/`map_split`) only rewraps a reference, so a guard
    //    projected onto a field of any payload stays exactly pointer-sized with
    //    its niche intact — the zero-cost claim holds across projection. ──
    assert!(size_of::<MelinoeRef<'static, 'static, u8>>() == size_of::<&u8>());
    assert!(size_of::<MelinoeMut<'static, 'static, u8>>() == size_of::<&mut u8>());

    // ── A writer shard is exactly its underlying `&mut [MelinoeCell]` slice
    //    reference (a fat pointer): the partition capability adds no footprint. ──
    assert!(
        size_of::<WriterShard<'static, 'static, u64>>()
            == size_of::<&mut [MelinoeCell<'static, u64>]>()
    );

    // ── A branded atomic is exactly its underlying atomic (brand marker is ZST). ──
    assert!(size_of::<BrandedAtomic<'static, AtomicUsize>>() == size_of::<AtomicUsize>());
    assert!(align_of::<BrandedAtomic<'static, AtomicUsize>>() == align_of::<AtomicUsize>());

    // ── Ordering and Cow policies are ZSTs: they route strategy at compile time. ──
    assert!(size_of::<Relaxed>() == 0);
    assert!(size_of::<AcqRel>() == 0);
    assert!(size_of::<SeqCst>() == 0);

    #[cfg(feature = "alloc")]
    {
        assert!(size_of::<Borrowed>() == 0);
        assert!(size_of::<Retained>() == 0);
    }
};

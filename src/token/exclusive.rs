//! [`ExclusiveToken`] — the unique, move-only owner of a brand.

use crate::token::brand_owner_token;

brand_owner_token! {
    /// The single, un-clonable owner of a brand's access rights.
    ///
    /// At most one `ExclusiveToken<'brand>` exists per brand (guaranteed by
    /// [`brand_scope`](crate::brand_scope)). Because the type is **move-only**—it
    /// deliberately implements neither [`Clone`] nor [`Copy`]—the borrow checker's
    /// aliasing rules on this one value transitively police *all* cells of the
    /// brand:
    ///
    /// * a shared borrow `&ExclusiveToken` is a [`ReadPermit`], and
    /// * an exclusive borrow `&mut ExclusiveToken` is a [`WritePermit`].
    ///
    /// Since you cannot hold `&mut` and `&` to the same token at once, you cannot
    /// hold a write permit and any read permit of the same brand at once. The XOR
    /// discipline `T xor &mut T xor &T` is thereby lifted from a single token to an
    /// entire region of branded cells at zero runtime cost.
    ///
    /// `ExclusiveToken` is `Send + Sync`: it is a ZST whose only field is a
    /// function-pointer phantom, so it may be moved to another thread to transfer
    /// write capability across a thread boundary.
    ///
    /// > *In myth Melinoë leads a train of restless phantoms; the exclusive token
    /// > is the one shade permitted to disturb the dead.*
    name: ExclusiveToken;
    /// Token-family selector for the standard exclusive brand scope.
    family: ExclusiveFamily;
    marker_extra: ();
    marker_init: ();
    debug: "ExclusiveToken<'brand>";
    share: yes;
}

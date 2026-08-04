# 7. Cow at the Ownership Boundary

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - CellCowExt::borrow_cow(tok) → Cow<'_, [T]>: borrowed path is zero-copy;
    owned path clones once when the caller needs an owned slice
  - borrow_cow_with(tok, policy): policy is a sealed ZST; Borrowed forces the
    borrowed path (zero-copy, Cow::Borrowed), Retained forces the owned path
    (one clone, Cow::Owned); RetainDecision covers data-dependent choices
  - Zero-copy borrow: Cow::Borrowed(&[T]) is a view of the cell slice through
    the shared token; no allocation, no copy
  - The ownership boundary pattern: a library that needs an owned buffer falls
    back to Cow::Owned; one that only reads uses Cow::Borrowed; the policy ZST
    selects the path at compile time with no runtime branch
  - Requires the `alloc` feature (Cow<'_, [T]> needs alloc::borrow::Cow)
-->

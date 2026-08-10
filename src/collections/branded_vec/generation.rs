use super::BrandedVec;
use crate::{brand_scope, ExclusiveToken};

/// Generate a branded vector and run a closure while its fresh brand is live.
///
/// This is the ergonomic generativity boundary for collection-backed work:
/// [`brand_scope`](crate::brand_scope) mints an invariant higher-ranked brand,
/// the generator fills storage under that brand, and the callback receives both
/// the vector and its unique [`ExclusiveToken`](crate::ExclusiveToken). The
/// callback's return value may escape, but the branded vector or token cannot.
///
/// The generator itself runs sequentially because it constructs the owned
/// collection. Use [`BrandedVec::partition_for_each_mut_with`] inside `f` when
/// the generated region needs a parallel mutation phase.
///
/// # Examples
///
/// ```
/// use melinoe::collections::with_generated;
///
/// let checksum = with_generated(8, |index| index * index, |values, token| {
///     values.as_slice(&token).iter().sum::<usize>()
/// });
/// assert_eq!(checksum, 140);
/// ```
///
/// With `std`, the callback can instead call
/// [`BrandedVec::partition_for_each_mut_with`] for a parallel mutation phase
/// while the fresh brand remains scoped to the callback.
#[inline]
pub fn with_generated<T, R, G, F>(len: usize, mut generate: G, f: F) -> R
where
    G: FnMut(usize) -> T,
    F: for<'brand> FnOnce(BrandedVec<'brand, T>, ExclusiveToken<'brand>) -> R,
{
    brand_scope(|token| f(BrandedVec::from_fn(len, &mut generate), token))
}

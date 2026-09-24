//! [`ExclusiveToken`] - the unique, move-only owner of a brand.

use crate::token::brand::{define_brand_owner_token, ExclusiveMarker};


define_brand_owner_token!(ExclusiveToken, ExclusiveFamily, ExclusiveMarker, "ExclusiveToken<'brand>");

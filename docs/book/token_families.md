# 2. Token Families

*Chapter prose deferred — DoR item.*

<!-- Key points to cover:
  - ExclusiveToken<'brand>: one per brand; Send + Sync; read + write; `&mut` hold
  - SharedReadToken<'brand, 'scope>: Copy; one per read-fan-out; `&` hold
  - ThreadLocalToken<'brand>: neither Send nor Sync; thread-confined owner
  - SyncRegionToken<'brand>: Send + Sync; thread-portable write capability
  - ReadPermit / WritePermit: sealed traits over the token types; MelinoeCell
    bounds on these rather than the concrete token types
  - token.share() → SharedReadToken: coerces from ExclusiveToken; Copy so
    multiple readers can fan out from one exclusive owner
-->

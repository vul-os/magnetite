//! The entitlement gate: a paid bundle serves **no** byte without a verified
//! receipt.
//!
//! ALIGNMENT.md §5 says `verify_receipt_for_item` "already covers most of it",
//! and §4 says why it must fail closed: "Magnetite's entitlement model is 'the
//! receipt *is* the entitlement', verified by re-reading the chain and failing
//! closed." The seam repeats it on the trait method itself — *"Verify a
//! receipt. **Must fail closed**: any doubt returns `false`."*
//!
//! Fail-open here does not degrade gracefully; it gives away the paid
//! catalogue. Every branch below that cannot *prove* entitlement returns
//! [`Verdict::Refused`], including the branches that are the operator's fault
//! (no rail configured, rail unreachable). A misconfigured node must serve
//! nothing rather than serve everything.
//!
//! # What this gate does not do
//!
//! It checks that a receipt is valid for an item. It does **not** establish that
//! the person presenting the receipt is the person who bought it, unless the
//! caller supplies an authenticated identity — see [`Requester`]. With
//! [`Requester::Anonymous`] a receipt is a bearer token: whoever holds the bytes
//! can replay them. That is a real limitation, stated here rather than
//! discovered later, and closing it is the `Identity` seam's job (ALIGNMENT.md
//! §7 phase 1 item 1), not this crate's.

use magnetite_seams::identity::PubKey;
use magnetite_seams::payment::{PaymentRail, Receipt, Settlement};

use crate::manifest::Pricing;

/// Who is asking, if anyone knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requester {
    /// An authenticated session, from the `Identity` seam. The receipt's
    /// `buyer` must equal this key, so a receipt lifted from someone else's
    /// traffic is inert.
    Authenticated(PubKey),
    /// No session. The receipt alone is the credential — a bearer token. Usable,
    /// and the only option until the identity seam is wired to this path, but
    /// understand that a leaked receipt is a leaked entitlement.
    Anonymous,
}

/// What the client presented.
#[derive(Clone, Debug, Default)]
pub struct Credentials {
    /// The receipt, if one was presented.
    pub receipt: Option<Receipt>,
    /// The authenticated requester, if a session exists.
    pub requester: Option<PubKey>,
}

impl Credentials {
    /// No credentials at all.
    pub fn none() -> Self {
        Self::default()
    }
    /// A bearer receipt with no session behind it.
    pub fn bearer(receipt: Receipt) -> Self {
        Self {
            receipt: Some(receipt),
            requester: None,
        }
    }
    /// A receipt presented by an authenticated key.
    pub fn authenticated(receipt: Receipt, requester: PubKey) -> Self {
        Self {
            receipt: Some(receipt),
            requester: Some(requester),
        }
    }
    fn who(&self) -> Requester {
        match self.requester {
            Some(k) => Requester::Authenticated(k),
            None => Requester::Anonymous,
        }
    }
}

/// Why access was refused. Each maps to exactly one HTTP status in
/// [`crate::respond`], and each is distinguishable in logs — an operator
/// debugging "nobody can buy my game" needs [`Self::NoRail`] not to look like
/// [`Self::ReceiptRejected`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The bundle is paid and no receipt was presented. → `402`.
    NoReceipt,
    /// The bundle is paid and no payment rail is configured, so entitlement
    /// **cannot be evaluated**. Fail closed: refuse. → `503`.
    ///
    /// The tempting alternative — "no rail configured, so treat everything as
    /// free" — is the exact fail-open that gives away paid content, and it is
    /// reachable by nothing more than a missing environment variable.
    NoRail,
    /// A receipt was presented and `verify_receipt_for_item` returned `false`:
    /// bad signature, wrong item, absent chain binding, or the rail could not
    /// re-read the chain. The rail does not distinguish these, and neither do
    /// we — "any doubt returns false" collapses them all. → `403`.
    ReceiptRejected,
    /// The receipt is valid but was bought by a different key than the
    /// authenticated requester. → `403`.
    WrongBuyer,
    /// A14: the rail reported [`Settlement::SignedUnsettled`] — the receipt's
    /// signature and arithmetic check out locally, but the rail could not
    /// re-confirm it against a chain right now (no network, most commonly).
    /// This node has **no serving policy yet** for an unsettled receipt (see
    /// [`Verdict::GrantedUnsettled`]'s docs — that is deliberately future
    /// work, the same way backlog item A15 is), so today it refuses rather
    /// than guess. → `403`.
    PendingSettlement,
}

impl Refusal {
    /// The HTTP status this refusal must produce.
    pub fn status(&self) -> u16 {
        match self {
            Self::NoReceipt => 402,
            Self::NoRail => 503,
            Self::ReceiptRejected | Self::WrongBuyer | Self::PendingSettlement => 403,
        }
    }
    /// A short, non-leaky reason for the response body and logs.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::NoReceipt => "no receipt presented for a paid bundle",
            Self::NoRail => "no payment rail configured; entitlement cannot be verified",
            Self::ReceiptRejected => "receipt did not verify for this item",
            Self::WrongBuyer => "receipt was not issued to the authenticated requester",
            Self::PendingSettlement => {
                "receipt verified locally but chain confirmation is unavailable; \
                 this node has no policy for serving on an unsettled receipt"
            }
        }
    }
}

/// The gate's answer.
///
/// A14: [`Self::Granted`] and [`Self::GrantedUnsettled`] are **two distinct
/// variants**, not one `Granted` carrying a `bool` or an `Option` a caller
/// could ignore. [`crate::respond::HostedBundle::respond`]'s `match` on this enum
/// has to name both arms — the compiler refuses to compile a `match` that
/// silently falls through an unsettled receipt as if it were settled. There is
/// deliberately no `is_granted()` collapsing both into one boolean; that
/// method existed once and was removed for exactly this reason (see
/// `git log` on this file) — it is the shape this crate's honesty rule ("a
/// caller must not be able to conflate signed-but-unsettled with settled by
/// accident") requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Serve the bytes: free, or a receipt the rail chain-verified (or had no
    /// chain to verify against at all — see [`Settlement::Settled`]).
    Granted,
    /// The receipt verified **locally** (signature + arithmetic) but the rail
    /// could not confirm it against a chain right now
    /// ([`Settlement::SignedUnsettled`]). Not [`Self::Granted`] — a caller
    /// that wants to serve provisionally on this tier must say so
    /// explicitly, by matching this arm; there is no default that does it
    /// for them.
    GrantedUnsettled,
    /// Serve nothing.
    Refused(Refusal),
}

/// Decide whether `creds` entitle the holder to a bundle priced as `pricing`.
///
/// `rail` is `Option` on purpose: a node hosting only free bundles has no reason
/// to configure a payment rail, and the type should say so rather than force a
/// mock in. The cost is that `Pricing::Paid` with `rail: None` must refuse, and
/// it does — [`Refusal::NoRail`].
///
/// Free bundles short-circuit before touching the rail, so hosting a free
/// catalogue never depends on a chain, an RPC endpoint or a network. That is
/// ALIGNMENT.md §1's "MUST function completely with zero coordinators" holding
/// at the level of a single function.
pub fn evaluate<R: PaymentRail + ?Sized>(
    pricing: &Pricing,
    rail: Option<&R>,
    creds: &Credentials,
) -> Verdict {
    let item = match pricing {
        Pricing::Free => return Verdict::Granted,
        Pricing::Paid { item } => item,
    };
    let Some(rail) = rail else {
        return Verdict::Refused(Refusal::NoRail);
    };
    let Some(receipt) = creds.receipt.as_ref() else {
        return Verdict::Refused(Refusal::NoReceipt);
    };
    // A14: the seam's own fail-closed, tier-distinguishing check — signature,
    // item binding, and (when the rail can reach it) a re-read of the chain.
    // We add nothing to it and subtract nothing from it — in particular we
    // never fall back to "the signature looked fine" when the chain check is
    // the part that failed, and we never collapse `SignedUnsettled` into
    // `Settled`: they return through different `Verdict` variants below.
    let settlement = match rail.verify_receipt_for_item_tiered(receipt, item) {
        None => return Verdict::Refused(Refusal::ReceiptRejected),
        Some(s) => s,
    };
    // Bind the receipt to the session, when there is one. Checked at both
    // tiers — a wrong buyer is refused regardless of whether the chain was
    // reachable.
    if let Requester::Authenticated(key) = creds.who() {
        if receipt.buyer != key {
            return Verdict::Refused(Refusal::WrongBuyer);
        }
    }
    match settlement {
        Settlement::Settled => Verdict::Granted,
        Settlement::SignedUnsettled => Verdict::GrantedUnsettled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::identity::RawKeypairAuth;
    use magnetite_seams::payment::{MockPaymentRail, PaymentSplit};

    fn key(seed: u8) -> PubKey {
        RawKeypairAuth::from_seed([seed; 32]).node_pubkey()
    }

    async fn receipt_for(rail: &MockPaymentRail, buyer: PubKey) -> Receipt {
        rail.checkout(&buyer, PaymentSplit::to_developer(key(200), 500))
            .await
    }

    #[tokio::test]
    async fn free_bundles_need_no_rail_and_no_receipt() {
        let none: Option<&MockPaymentRail> = None;
        assert_eq!(
            evaluate(&Pricing::Free, none, &Credentials::none()),
            Verdict::Granted
        );
    }

    #[tokio::test]
    async fn paid_bundle_with_no_receipt_is_refused() {
        let rail = MockPaymentRail::new();
        let v = evaluate(
            &Pricing::Paid {
                item: "game:x".into(),
            },
            Some(&rail),
            &Credentials::none(),
        );
        assert_eq!(v, Verdict::Refused(Refusal::NoReceipt));
        assert_eq!(Refusal::NoReceipt.status(), 402);
    }

    /// The fail-open that would give away the paid catalogue: a node with no
    /// rail configured must serve nothing, not everything.
    #[tokio::test]
    async fn paid_bundle_with_no_rail_fails_closed() {
        let none: Option<&MockPaymentRail> = None;
        let rail = MockPaymentRail::new();
        let r = receipt_for(&rail, key(1)).await;
        for creds in [Credentials::none(), Credentials::bearer(r)] {
            assert_eq!(
                evaluate(
                    &Pricing::Paid {
                        item: "game:x".into()
                    },
                    none,
                    &creds
                ),
                Verdict::Refused(Refusal::NoRail),
                "a missing rail must never read as 'free'"
            );
        }
        assert_eq!(Refusal::NoRail.status(), 503);
    }

    #[tokio::test]
    async fn a_verified_receipt_grants() {
        let rail = MockPaymentRail::new();
        let buyer = key(1);
        let r = receipt_for(&rail, buyer).await;
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::bearer(r.clone())
            ),
            Verdict::Granted
        );
        // And with the session binding satisfied.
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::authenticated(r, buyer)
            ),
            Verdict::Granted
        );
    }

    #[tokio::test]
    async fn a_tampered_receipt_is_refused() {
        let rail = MockPaymentRail::new();
        let mut r = receipt_for(&rail, key(1)).await;
        r.total += 1; // signature no longer covers the bytes
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::bearer(r)
            ),
            Verdict::Refused(Refusal::ReceiptRejected)
        );
    }

    #[tokio::test]
    async fn a_receipt_for_someone_else_is_refused_when_a_session_exists() {
        let rail = MockPaymentRail::new();
        let r = receipt_for(&rail, key(1)).await;
        // Same receipt, different authenticated requester.
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::authenticated(r.clone(), key(2))
            ),
            Verdict::Refused(Refusal::WrongBuyer)
        );
        // Without a session the same receipt is a bearer token and does pass —
        // which is exactly the limitation documented on `Requester::Anonymous`.
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::bearer(r)
            ),
            Verdict::Granted
        );
    }

    /// A rail whose item binding is real: `MockPaymentRail` ignores `item`
    /// (its receipts are bound by the caller's database, per the seam's default
    /// impl), so an item-sensitive rail is stubbed here to prove the gate
    /// actually routes the item string through and honours a `false`.
    #[tokio::test]
    async fn the_item_string_is_honoured() {
        struct OneItemRail(&'static str);
        #[async_trait::async_trait]
        impl PaymentRail for OneItemRail {
            async fn checkout(&self, _b: &PubKey, _s: PaymentSplit) -> Receipt {
                unreachable!("gate never charges")
            }
            async fn open_channel(
                &self,
                _p: &PubKey,
            ) -> Result<magnetite_seams::payment::Channel, magnetite_seams::payment::PaymentError>
            {
                unreachable!()
            }
            async fn escrow(
                &self,
                _t: magnetite_seams::payment::WagerTerms,
            ) -> Result<magnetite_seams::payment::Escrow, magnetite_seams::payment::PaymentError>
            {
                unreachable!()
            }
            fn verify_receipt(&self, _r: &Receipt) -> bool {
                true
            }
            fn verify_receipt_for_item(&self, _r: &Receipt, item: &str) -> bool {
                item == self.0
            }
        }

        let rail = OneItemRail("game:right");
        let mock = MockPaymentRail::new();
        let r = receipt_for(&mock, key(1)).await;

        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:right".into()
                },
                Some(&rail),
                &Credentials::bearer(r.clone())
            ),
            Verdict::Granted
        );
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:wrong".into()
                },
                Some(&rail),
                &Credentials::bearer(r)
            ),
            Verdict::Refused(Refusal::ReceiptRejected),
            "a receipt for another item must not unlock this bundle"
        );
    }

    // ── A14: Settlement tiers reach `Verdict` distinctly ────────────────────

    /// A rail whose `verify_receipt_for_item_tiered` genuinely distinguishes
    /// the two tiers, standing in for `magnetite_solana_rail::SolanaPaymentRail`
    /// with its network switched off. `verify_receipt`/`verify_receipt_for_item`
    /// (the untiered methods `evaluate` no longer calls, but which must still
    /// exist to satisfy the trait) simply delegate to the mock underneath.
    struct TieredRail {
        inner: MockPaymentRail,
        settlement: Option<Settlement>,
    }

    #[async_trait::async_trait]
    impl PaymentRail for TieredRail {
        async fn checkout(&self, _b: &PubKey, _s: PaymentSplit) -> Receipt {
            unreachable!("gate never charges")
        }
        async fn open_channel(
            &self,
            _p: &PubKey,
        ) -> Result<magnetite_seams::payment::Channel, magnetite_seams::payment::PaymentError>
        {
            unreachable!()
        }
        async fn escrow(
            &self,
            _t: magnetite_seams::payment::WagerTerms,
        ) -> Result<magnetite_seams::payment::Escrow, magnetite_seams::payment::PaymentError>
        {
            unreachable!()
        }
        fn verify_receipt(&self, r: &Receipt) -> bool {
            self.inner.verify_receipt(r)
        }
        fn verify_receipt_for_item(&self, r: &Receipt, item: &str) -> bool {
            self.inner.verify_receipt_for_item(r, item)
        }
        fn verify_receipt_for_item_tiered(&self, _r: &Receipt, _item: &str) -> Option<Settlement> {
            self.settlement
        }
    }

    #[tokio::test]
    async fn a_settled_receipt_grants_the_settled_verdict() {
        let mock = MockPaymentRail::new();
        let buyer = key(1);
        let r = receipt_for(&mock, buyer).await;
        let rail = TieredRail {
            inner: mock,
            settlement: Some(Settlement::Settled),
        };
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::bearer(r)
            ),
            Verdict::Granted
        );
    }

    /// The non-conflation guard: a signed-but-unsettled receipt must reach
    /// `Verdict::GrantedUnsettled`, a value distinct from `Verdict::Granted` —
    /// never the same variant, never silently upgraded.
    #[tokio::test]
    async fn a_signed_unsettled_receipt_grants_the_unsettled_verdict_not_granted() {
        let mock = MockPaymentRail::new();
        let buyer = key(2);
        let r = receipt_for(&mock, buyer).await;
        let rail = TieredRail {
            inner: mock,
            settlement: Some(Settlement::SignedUnsettled),
        };
        let v = evaluate(
            &Pricing::Paid {
                item: "game:x".into(),
            },
            Some(&rail),
            &Credentials::bearer(r),
        );
        assert_eq!(v, Verdict::GrantedUnsettled);
        assert_ne!(
            v,
            Verdict::Granted,
            "signed-but-unsettled must never equal fully granted"
        );
    }

    #[tokio::test]
    async fn a_refused_tiered_result_still_refuses() {
        let mock = MockPaymentRail::new();
        let buyer = key(3);
        let r = receipt_for(&mock, buyer).await;
        let rail = TieredRail {
            inner: mock,
            settlement: None,
        };
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::bearer(r)
            ),
            Verdict::Refused(Refusal::ReceiptRejected)
        );
    }

    /// The wrong-buyer check runs at both tiers — a signed-but-unsettled
    /// receipt bought by someone else must not slip through as either kind of
    /// grant.
    #[tokio::test]
    async fn wrong_buyer_is_refused_even_when_unsettled() {
        let mock = MockPaymentRail::new();
        let buyer = key(4);
        let r = receipt_for(&mock, buyer).await;
        let rail = TieredRail {
            inner: mock,
            settlement: Some(Settlement::SignedUnsettled),
        };
        assert_eq!(
            evaluate(
                &Pricing::Paid {
                    item: "game:x".into()
                },
                Some(&rail),
                &Credentials::authenticated(r, key(5))
            ),
            Verdict::Refused(Refusal::WrongBuyer)
        );
    }

    /// `Refusal::PendingSettlement` (the HTTP-layer name for an unsettled
    /// receipt this node has no policy to serve on) must be distinguishable
    /// from `ReceiptRejected` in both status and reason — an operator
    /// debugging "my receipt won't work" needs to see "pending", not
    /// "rejected", when the truth is the former.
    #[test]
    fn pending_settlement_is_a_distinct_refusal_not_a_rejected_alias() {
        assert_eq!(Refusal::PendingSettlement.status(), 403);
        assert_ne!(
            Refusal::PendingSettlement.reason(),
            Refusal::ReceiptRejected.reason()
        );
    }
}

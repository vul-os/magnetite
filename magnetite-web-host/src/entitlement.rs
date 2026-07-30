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
use magnetite_seams::payment::{PaymentRail, Receipt};

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
}

impl Refusal {
    /// The HTTP status this refusal must produce.
    pub fn status(&self) -> u16 {
        match self {
            Self::NoReceipt => 402,
            Self::NoRail => 503,
            Self::ReceiptRejected | Self::WrongBuyer => 403,
        }
    }
    /// A short, non-leaky reason for the response body and logs.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::NoReceipt => "no receipt presented for a paid bundle",
            Self::NoRail => "no payment rail configured; entitlement cannot be verified",
            Self::ReceiptRejected => "receipt did not verify for this item",
            Self::WrongBuyer => "receipt was not issued to the authenticated requester",
        }
    }
}

/// The gate's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Serve the bytes.
    Granted,
    /// Serve nothing.
    Refused(Refusal),
}

impl Verdict {
    /// Whether bytes may be served.
    pub fn is_granted(&self) -> bool {
        matches!(self, Self::Granted)
    }
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
    // The seam's own fail-closed check: signature, item binding, and for chain
    // rails a re-read of the chain. We add nothing to it and subtract nothing
    // from it — in particular we never fall back to "the signature looked fine"
    // when the item check is the part that failed.
    if !rail.verify_receipt_for_item(receipt, item) {
        return Verdict::Refused(Refusal::ReceiptRejected);
    }
    // Bind the receipt to the session, when there is one.
    if let Requester::Authenticated(key) = creds.who() {
        if receipt.buyer != key {
            return Verdict::Refused(Refusal::WrongBuyer);
        }
    }
    Verdict::Granted
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_seams::identity::RawKeypairAuth;
    use magnetite_seams::payment::{MockPaymentRail, PaymentSplit, Split};

    fn key(seed: u8) -> PubKey {
        RawKeypairAuth::from_seed([seed; 32]).node_pubkey()
    }

    async fn receipt_for(rail: &MockPaymentRail, buyer: PubKey) -> Receipt {
        rail.checkout(
            &buyer,
            PaymentSplit {
                developer: Split {
                    wallet: key(200),
                    amount: 500,
                },
                operator: None,
                protocol_fee_bps: 0,
            },
        )
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
        assert!(evaluate(
            &Pricing::Paid {
                item: "game:x".into()
            },
            Some(&rail),
            &Credentials::bearer(r)
        )
        .is_granted());
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

        assert!(evaluate(
            &Pricing::Paid {
                item: "game:right".into()
            },
            Some(&rail),
            &Credentials::bearer(r.clone())
        )
        .is_granted());
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
}

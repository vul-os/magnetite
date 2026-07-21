//! Wallet tests — NON-CUSTODIAL model (DECENTRALIZATION.md §2 + §3.6).
//!
//! The old suite tested a custodial USD balance, Paystack deposits and Wise
//! withdrawals. None of that exists any more: a wallet is an *address*, a purchase
//! is a wallet→wallet checkout on the `PaymentRail` seam, and the signed `Receipt`
//! is the entitlement.
//!
//! Everything here runs OFFLINE against `MockPaymentRail` — no DB, no network.

#[cfg(test)]
mod noncustodial_wallet_tests {
    use magnetite_backend::api::wallet::{LinkWalletRequest, LinkedWallet};
    use magnetite_backend::services::payment::{
        rail, sale_split, units_from_usd, verify_receipt, PaymentRail, PubKey,
    };
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    #[test]
    fn linked_wallet_reports_an_address_and_never_a_balance() {
        let wallet = LinkedWallet {
            user_id: Uuid::new_v4(),
            wallet_address: Some(PubKey([0xAB; 32]).to_hex()),
            custodial: false,
            rail: "mock".to_string(),
        };

        let json = serde_json::to_string(&wallet).unwrap();
        assert!(json.contains("wallet_address"));
        assert!(json.contains("\"custodial\":false"));
        assert!(
            !json.contains("balance"),
            "a non-custodial wallet must never expose a balance: {json}"
        );
    }

    #[test]
    fn link_request_accepts_hex_pubkey() {
        let key = PubKey([0x11; 32]).to_hex();
        let json = format!(r#"{{ "wallet_address": "{key}" }}"#);
        let req: LinkWalletRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.wallet_address, key);
        assert!(PubKey::from_hex(&req.wallet_address).is_ok());
    }

    #[test]
    fn link_request_rejects_garbage_pubkey() {
        assert!(PubKey::from_hex("not-a-key").is_err());
        assert!(PubKey::from_hex("dead").is_err(), "wrong length rejected");
    }

    /// checkout → receipt → (would-be) entitlement, entirely offline.
    #[tokio::test]
    async fn checkout_receipt_gates_entitlement() {
        let buyer = PubKey([0xB0; 32]);
        let developer = PubKey([0xD0; 32]);
        let amount = units_from_usd(dec!(19.99));

        let receipt = rail()
            .checkout(&buyer, sale_split(developer, amount, None))
            .await;

        assert_eq!(receipt.buyer, buyer);
        assert_eq!(receipt.total, 1999);
        assert_eq!(receipt.protocol_fee, 0, "protocol fee defaults to 0 bps");
        assert_eq!(receipt.payouts[0].wallet, developer);
        assert!(
            verify_receipt(&receipt),
            "a fresh receipt must verify — this is what grants the entitlement"
        );
    }

    /// The receipt is the entitlement, so forging it must fail closed.
    #[tokio::test]
    async fn tampered_receipt_never_grants_entitlement() {
        let buyer = PubKey([0xB1; 32]);
        let mut receipt = rail()
            .checkout(&buyer, sale_split(PubKey([0xD1; 32]), 5000, None))
            .await;
        assert!(verify_receipt(&receipt));

        // Redirect the payout to an attacker wallet.
        receipt.payouts[0].wallet = PubKey([0xEE; 32]);
        assert!(!verify_receipt(&receipt), "forged payee must be rejected");
    }

    /// Hosting fees (§3.6b) ride a payment channel; the mock rail is deterministic.
    #[tokio::test]
    async fn hosting_channel_is_deterministic_and_offline() {
        let operator = PubKey([0x0B; 32]);
        let a = rail().open_channel(&operator).await.unwrap();
        let b = rail().open_channel(&operator).await.unwrap();
        assert_eq!(a.id, b.id, "channel id must be deterministic");
        assert_eq!(a.peer, operator);
    }

    #[test]
    fn usd_prices_convert_to_rail_units() {
        assert_eq!(units_from_usd(dec!(0.01)), 1);
        assert_eq!(units_from_usd(dec!(25.00)), 2500);
        assert_eq!(units_from_usd(dec!(1234.56)), 123_456);
    }
}

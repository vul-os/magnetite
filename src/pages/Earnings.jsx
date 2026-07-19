import { useState, useEffect, useMemo } from 'react';
import Layout from '../components/Layout';
import AnalyticsChart from '../components/charts/AnalyticsChart';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './Earnings.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

/**
 * Developer revenue is **receipt-backed** (§3.6 PaymentRail). A purchase moves USDC
 * from the buyer's wallet straight into the developer's wallet in one atomic
 * transfer and emits a signed receipt. There is no custody, no balance we hold, and
 * therefore nothing to withdraw — this page is a ledger view, not a bank account.
 */

// Mock data — only used when VITE_USE_MOCKS === 'true'
const MOCK_RECEIPTS = [
  { id: 'rcpt_01HZM4A7QP3D9K2X', buyer_pubkey: 'ed25519:8f21c0aa47be1d93', game_title: 'Cosmic Raiders', item_name: 'Plasma Rifle Skin',   total: 0.99, protocol_fee: 0, rail_pubkey: 'ed25519:9f3c7a1e4b82d05c', settled_at: '2026-05-18T14:32:00Z', voided: false },
  { id: 'rcpt_01HZM2X8ND6F4T0B', buyer_pubkey: 'ed25519:1d74be92f0c38a5e', game_title: 'Cosmic Raiders', item_name: 'Void Shield Pack',    total: 1.99, protocol_fee: 0, rail_pubkey: 'ed25519:9f3c7a1e4b82d05c', settled_at: '2026-05-18T11:07:00Z', voided: false },
  { id: 'rcpt_01HZKZ5R2W8H7YJC', buyer_pubkey: 'ed25519:c93a01fe5b26d7f4', game_title: 'Speed Legends',  item_name: 'Carbon Livery',       total: 1.49, protocol_fee: 0, rail_pubkey: 'ed25519:2b6d90fe1a47c3b8', settled_at: '2026-05-17T18:44:00Z', voided: false },
  { id: 'rcpt_01HZKQ9M1V4B6PLD', buyer_pubkey: 'ed25519:5e08d3ba71c94f26', game_title: 'Speed Legends',  item_name: 'Neon Trail Effect',   total: 0.79, protocol_fee: 0, rail_pubkey: 'ed25519:2b6d90fe1a47c3b8', settled_at: '2026-05-17T09:21:00Z', voided: false },
  { id: 'rcpt_01HZKC3T7Y2N5RQA', buyer_pubkey: 'ed25519:a2f6714c8e03bd51', game_title: 'Cosmic Raiders', item_name: 'XP Accelerator (7d)', total: 0.49, protocol_fee: 0, rail_pubkey: 'ed25519:9f3c7a1e4b82d05c', settled_at: '2026-05-16T20:58:00Z', voided: true  },
  { id: 'rcpt_01HZK08F6J1C3WMZ', buyer_pubkey: 'ed25519:63bd47a1c05e29fd', game_title: 'Cosmic Raiders', item_name: 'Plasma Rifle Skin',   total: 0.99, protocol_fee: 0, rail_pubkey: 'ed25519:9f3c7a1e4b82d05c', settled_at: '2026-05-16T08:12:00Z', voided: false },
];

const MOCK_REVENUE_SERIES = [
  { date: '2026-05-12', value: 18.42 },
  { date: '2026-05-13', value: 24.90 },
  { date: '2026-05-14', value: 21.15 },
  { date: '2026-05-15', value: 33.68 },
  { date: '2026-05-16', value: 29.04 },
  { date: '2026-05-17', value: 41.27 },
  { date: '2026-05-18', value: 37.55 },
];

const shortKey = (pk) => {
  if (!pk) return '—';
  const raw = String(pk).replace(/^[a-z0-9]+:/i, '');
  return raw.length <= 12 ? raw : `${raw.slice(0, 6)}…${raw.slice(-4)}`;
};

const usdc = (amount) =>
  `${Number(amount ?? 0).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDC`;

export default function Earnings() {
  const { t } = useTranslation();
  const [receipts, setReceipts]   = useState(USE_MOCKS ? MOCK_RECEIPTS : []);
  const [series, setSeries]       = useState(USE_MOCKS ? MOCK_REVENUE_SERIES : []);
  const [feeBps, setFeeBps]       = useState(0);
  const [walletPubkey, setWalletPubkey] = useState(USE_MOCKS ? 'ed25519:4a7f21c9e08b3d65' : null);
  const [loading, setLoading]     = useState(!USE_MOCKS);
  const [loadError, setLoadError] = useState(null);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function loadData() {
      setLoading(true);
      setLoadError(null);
      try {
        const res = await api.developer.earnings();
        if (cancelled) return;

        const d = res?.data ?? res ?? {};
        const list = Array.isArray(d.receipts) ? d.receipts
          : Array.isArray(d.items) ? d.items
          : Array.isArray(d) ? d : [];
        setReceipts(list);

        if (Array.isArray(d.revenue_series)) setSeries(d.revenue_series);
        if (d.protocol_fee_bps != null) setFeeBps(Number(d.protocol_fee_bps));
        if (d.wallet_pubkey) setWalletPubkey(d.wallet_pubkey);
      } catch (err) {
        if (!cancelled) setLoadError(err.message || 'Failed to load revenue');
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadData();
    return () => { cancelled = true; };
  }, []);

  const totals = useMemo(() => {
    const live = receipts.filter(r => !r.voided);
    return {
      count: live.length,
      voided: receipts.length - live.length,
      received: live.reduce((sum, r) => sum + Number(r.total ?? 0) - Number(r.protocol_fee ?? 0), 0),
      fees: live.reduce((sum, r) => sum + Number(r.protocol_fee ?? 0), 0),
    };
  }, [receipts]);

  return (
    <Layout>
      <div className="earnings-page">
        <header className="earnings-header">
          <span className="kicker">// DEVELOPER REVENUE</span>
          <h1>{t('earnings.title')}</h1>
          <p className="earnings-subtitle">
            Every sale settles wallet-to-wallet in USDC and produces a signed receipt.
            Funds land in your wallet the moment a buyer checks out — there is nothing to withdraw.
          </p>
        </header>

        {loadError && (
          <div role="alert" style={{ padding: '0.75rem 1rem', marginBottom: '1.5rem', background: 'rgba(255,84,104,0.1)', border: '1px solid var(--color-error)', borderRadius: 'var(--radius)', color: 'var(--color-error)', fontSize: '0.875rem' }}>
            {loadError}
          </div>
        )}

        <div className="earnings-summary" aria-label="Revenue summary">
          <div className="summary-card primary">
            <span className="summary-icon" aria-hidden="true">◈</span>
            <div className="summary-content">
              <span className="summary-label">Received (USDC)</span>
              <span className="summary-value amber" aria-live="polite">
                {loading ? '—' : usdc(totals.received)}
              </span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">▤</span>
            <div className="summary-content">
              <span className="summary-label">
                Signed receipts{totals.voided > 0 ? ` · ${totals.voided} voided` : ''}
              </span>
              <span className="summary-value">{loading ? '—' : totals.count.toLocaleString()}</span>
            </div>
          </div>
          <div className="summary-card">
            <span className="summary-icon" aria-hidden="true">%</span>
            <div className="summary-content">
              <span className="summary-label">Protocol fee</span>
              <span className="summary-value">{feeBps} bps</span>
            </div>
          </div>
        </div>

        {/* ── Where the money goes ───────────────────────────────────────── */}
        <div className="settlement-section">
          <span className="kicker">// SETTLEMENT</span>
          <h3>Paid directly to your wallet</h3>
          <p className="settlement-note">
            You receive the <strong>full subtotal</strong> of every sale. The protocol fee is
            currently <strong>{feeBps} bps</strong> (configurable, default 0) and is deducted by the
            rail at checkout, not by an intermediary. There are no platform revenue splits, no
            custodial balance, no payout queue and no bank details to store.
          </p>
          <dl className="settlement-facts">
            <div>
              <dt>Destination wallet</dt>
              <dd className="mono">{walletPubkey ? shortKey(walletPubkey) : 'Not linked'}</dd>
            </div>
            <div>
              <dt>Rail</dt>
              <dd className="mono">USDC · non-custodial</dd>
            </div>
            <div>
              <dt>Settlement</dt>
              <dd className="mono">Atomic, at checkout</dd>
            </div>
          </dl>
        </div>

        {/* ── Revenue over time ──────────────────────────────────────────── */}
        <div className="revenue-chart-section">
          <span className="kicker">// REVENUE OVER TIME</span>
          <h3>Daily settled revenue</h3>
          <AnalyticsChart
            data={series}
            color="amber"
            gradientId="earnings-revenue"
            yFormatter={(v) => `${v}`}
            tooltipFormatter={(v) => usdc(v)}
            emptyMessage="No settled receipts in this period."
          />
        </div>

        {/* ── Receipts ledger ────────────────────────────────────────────── */}
        <div className="receipts-section">
          {loading ? (
            <div className="loading-state">
              <span className="spinner large" aria-hidden="true" />
              <span>{t('common.loading')}</span>
            </div>
          ) : receipts.length === 0 ? (
            <p style={{ padding: '2rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
              No receipts yet. Revenue appears here as soon as a buyer completes a wallet checkout.
            </p>
          ) : (
            <table className="receipts-table" aria-label="Signed revenue receipts">
              <thead>
                <tr>
                  <th scope="col">{t('earnings.colDate')}</th>
                  <th scope="col">Buyer</th>
                  <th scope="col">Game / Item</th>
                  <th scope="col">{t('earnings.colAmount')}</th>
                  <th scope="col">Protocol fee</th>
                  <th scope="col">Receipt</th>
                  <th scope="col">{t('earnings.colStatus')}</th>
                </tr>
              </thead>
              <tbody>
                {receipts.map(r => (
                  <tr key={r.id} className={r.voided ? 'voided' : ''}>
                    <td className="date-cell">
                      {r.settled_at ? String(r.settled_at).slice(0, 10) : '—'}
                    </td>
                    <td className="mono">{shortKey(r.buyer_pubkey)}</td>
                    <td>
                      <div className="receipt-item">
                        <span className="receipt-item-name">{r.item_name ?? '—'}</span>
                        <span className="receipt-item-game">{r.game_title ?? '—'}</span>
                      </div>
                    </td>
                    <td className={`amount-cell ${r.voided ? '' : 'positive'}`}>{usdc(r.total)}</td>
                    <td className="fee-cell">{usdc(r.protocol_fee)}</td>
                    <td>
                      <div className="receipt-ids">
                        <span className="mono" title={r.id}>{shortKey(r.id)}</span>
                        <span className="mono receipt-rail" title={r.rail_pubkey}>rail {shortKey(r.rail_pubkey)}</span>
                      </div>
                    </td>
                    <td>
                      <span className={`status-badge ${r.voided ? 'voided' : 'settled'}`}>
                        {r.voided ? 'Voided' : 'Settled'}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </Layout>
  );
}

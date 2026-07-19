import { useState, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
import { api } from '../../api/client';
import { formatUSDC, formatProtocolFee, shortKey } from '../../utils/currency';
import './admin.css';

/*
 * Admin finance — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
 *
 * This node never holds funds, so there is no float to reconcile, no pending
 * payout queue and no deposit ledger. What an admin can see is the stream of
 * *signed receipts* minted by checkout, and the protocol fee those receipts
 * carried (which defaults to 0 bps — the developer keeps the full subtotal).
 */

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

/** Protocol fee charged by this node, in basis points. Default: none. */
const PROTOCOL_FEE_BPS = 0;

function authFetch(endpoint, options = {}) {
  const token = localStorage.getItem('token');
  return fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });
}

const RAIL_PUBKEY = '3ac9017e5fb2d846902ce15b7a4d3f80c6e1927b5d0af348e2c76b91045fd8a2';

/* Mock data — only used when VITE_USE_MOCKS=true */
const MOCK_RECEIPTS = import.meta.env.VITE_USE_MOCKS
  ? [
      { id: 'rcpt_01HQ8ZK3NP', kind: 'item_purchase', buyer: 'CryptoGamer42', payee: '5c8de401f9b7236a0d14e8c93b750af26e1d3809c47ba62e91d70b385ac64f13', game: 'Cosmic Raiders',  total:  2.50, protocolFee: 0, rail: RAIL_PUBKEY, date: '2026-07-16 14:32', voided: false },
      { id: 'rcpt_01HQ7WX8TM', kind: 'hosting_fee',   buyer: 'NeonRacer99',   payee: '7e30b8a1cd94c25e06f381ba47d9e2c0518736fa9db4e18c02735d6ab91af4e2', game: 'Neon Drift',      total:  1.50, protocolFee: 0, rail: RAIL_PUBKEY, date: '2026-07-16 14:28', voided: false },
      { id: 'rcpt_01HQ5MC2QD', kind: 'tier',          buyer: 'PixelMaster',   payee: 'b0561ea38c7d29f4013a8ce65b27d09f4e81a6c37d520fb98e14c7302a6db85f', game: 'Galaxy Conquest', total:  9.99, protocolFee: 0, rail: RAIL_PUBKEY, date: '2026-07-15 13:45', voided: false },
      { id: 'rcpt_01HQ2FA9RB', kind: 'item_purchase', buyer: 'IndieDev_Mike', payee: 'd41b9c07a5e8f236104b7dd9ce8215af6390b7e04c2d18f5a6b93e70d1c4825f', game: 'Dungeon Realms',  total:  1.49, protocolFee: 0, rail: RAIL_PUBKEY, date: '2026-07-14 20:55', voided: true  },
    ]
  : null;

function normaliseReceipt(r) {
  return {
    id:          r.id,
    kind:        r.kind ?? r.tx_type ?? r.type ?? 'unknown',
    buyer:       r.username ?? r.buyer ?? r.user ?? 'Unknown',
    payee:       r.payee ?? r.counterparty ?? r.developer_pubkey ?? null,
    game:        r.game_title ?? r.game ?? '—',
    total:       Math.abs(parseFloat(r.total ?? r.amount ?? 0)),
    protocolFee: Math.abs(parseFloat(r.protocol_fee ?? 0)),
    rail:        r.rail_pubkey ?? r.rail ?? null,
    date:        r.created_at ? r.created_at.replace('T', ' ').slice(0, 16) : (r.date ?? ''),
    voided:      Boolean(r.voided ?? (r.status === 'refunded')),
  };
}

/** Human label for a receipt `kind`. */
function kindLabel(kind) {
  if (kind === 'item_purchase') return 'Item';
  if (kind === 'hosting_fee')   return 'Hosting';
  if (kind === 'tier')          return 'Tier';
  return 'Receipt';
}

export default function Finance() {
  const [receipts, setReceipts]           = useState(MOCK_RECEIPTS ?? []);
  const [loadingReceipts, setLoading]     = useState(!MOCK_RECEIPTS);
  const [error, setError]                 = useState(null);
  const [kindFilter, setKindFilter]       = useState('all');
  const [currentPage, setCurrentPage]     = useState(1);
  const [voidingReceipt, setVoidingReceipt] = useState(null);   // receipt being voided
  const [voidReason, setVoidReason]         = useState('');
  const [voidError, setVoidError]           = useState(null);
  const [voidSuccess, setVoidSuccess]       = useState(null);
  const perPage = 10;

  const fetchData = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) return;

    setLoading(true);
    setError(null);

    try {
      const res = await authFetch('/api/admin/transactions?limit=100');
      if (res.ok) {
        const d = await res.json();
        const raw = d.data ?? d ?? [];
        setReceipts(Array.isArray(raw) ? raw.map(normaliseReceipt) : []);
      }
    } catch (err) {
      setError(err.message || 'Failed to load receipts');
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch receipt data from the admin API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchData(); }, [fetchData]);

  const filteredReceipts = receipts.filter(r => {
    if (kindFilter === 'hosting') return r.kind === 'hosting_fee';
    if (kindFilter === 'items')   return r.kind === 'item_purchase';
    if (kindFilter === 'tiers')   return r.kind === 'tier';
    return true;
  });

  const paginatedReceipts = filteredReceipts.slice(
    (currentPage - 1) * perPage,
    currentPage * perPage
  );

  const liveReceipts    = receipts.filter(r => !r.voided);
  const grossSettled    = liveReceipts.reduce((sum, r) => sum + r.total, 0);
  const protocolFees    = liveReceipts.reduce((sum, r) => sum + r.protocolFee, 0);
  const toCounterparties = grossSettled - protocolFees;
  const voidedCount     = receipts.length - liveReceipts.length;

  const handleVoidConfirm = async () => {
    if (!voidingReceipt) return;
    setVoidError(null);
    try {
      await api.admin.refundTransaction(voidingReceipt.id, { reason: voidReason || undefined });
      setVoidSuccess(`Receipt ${String(voidingReceipt.id).slice(0, 12)} voided`);
      setVoidingReceipt(null);
      setVoidReason('');
      // Mark the receipt as voided in local state
      setReceipts(prev =>
        prev.map(r => r.id === voidingReceipt.id ? { ...r, voided: true } : r)
      );
    } catch (err) {
      setVoidError(err.message || 'Void failed');
    }
  };

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Platform Control</span>
              <h1>Finance Dashboard</h1>
              <p>Signed receipts and protocol-fee totals — this node holds no funds</p>
            </div>
          </header>

          {error && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {error}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={fetchData}>
                Retry
              </button>
            </div>
          )}

          {voidError && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              Void error: {voidError}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={() => setVoidError(null)}>
                Dismiss
              </button>
            </div>
          )}

          {voidSuccess && (
            <div className="admin-success-banner" role="status" style={{ padding: '0.75rem 1rem', marginBottom: '1rem', borderRadius: 'var(--radius)', background: 'rgba(34,197,94,0.1)', border: '1px solid rgba(34,197,94,0.3)', color: 'var(--color-success, #22c55e)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
              {voidSuccess}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={() => setVoidSuccess(null)}>
                Dismiss
              </button>
            </div>
          )}

          {/* Void confirmation modal */}
          {voidingReceipt && (
            <div
              role="dialog"
              aria-modal="true"
              aria-labelledby="void-dialog-title"
              style={{
                position: 'fixed', inset: 0, zIndex: 'var(--z-modal, 1000)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: 'rgba(0,0,0,0.6)', backdropFilter: 'blur(4px)',
              }}
            >
              <div style={{
                background: 'var(--color-bg-elevated)',
                border: '1px solid var(--color-border)',
                borderRadius: 'var(--radius-lg)',
                padding: '2rem',
                maxWidth: 420,
                width: '90%',
              }}>
                <h2 id="void-dialog-title" style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-base)', marginBottom: '0.5rem', color: 'var(--color-text-primary)' }}>
                  // CONFIRM VOID
                </h2>
                <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', marginBottom: '1.25rem' }}>
                  Void receipt <code style={{ fontFamily: 'var(--font-mono)', color: 'var(--color-accent)' }}>{String(voidingReceipt.id).slice(0, 12)}</code> for <strong>{formatUSDC(voidingReceipt.total)}</strong>, minted for <strong>{voidingReceipt.buyer}</strong>? This revokes the entitlement it backs. It does not move funds — settlement already happened wallet to wallet.
                </p>
                <label style={{ display: 'block', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.4rem' }}>
                  Reason (optional)
                </label>
                <textarea
                  value={voidReason}
                  onChange={e => setVoidReason(e.target.value)}
                  placeholder="e.g. duplicate receipt, disputed entitlement"
                  rows={2}
                  style={{
                    width: '100%', boxSizing: 'border-box',
                    background: 'var(--color-bg-surface)',
                    border: '1px solid var(--color-border)',
                    borderRadius: 'var(--radius)',
                    padding: '0.6rem 0.75rem',
                    color: 'var(--color-text-primary)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: 'var(--text-sm)',
                    resize: 'vertical',
                    marginBottom: '1.25rem',
                  }}
                />
                <div style={{ display: 'flex', gap: '0.75rem', justifyContent: 'flex-end' }}>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => { setVoidingReceipt(null); setVoidReason(''); setVoidError(null); }}
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={handleVoidConfirm}
                  >
                    Confirm Void
                  </Button>
                </div>
              </div>
            </div>
          )}

          <div className="admin-stats-grid">
            {loadingReceipts ? (
              Array.from({ length: 4 }).map((_, i) => (
                <div key={i} className="admin-stat-card skeleton-card" aria-busy="true">
                  <div className="skeleton skeleton-icon" />
                  <div className="admin-stat-info">
                    <div className="skeleton skeleton-label" />
                    <div className="skeleton skeleton-value" />
                  </div>
                </div>
              ))
            ) : (
              <>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">§</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Receipts Minted</span>
                    <span className="admin-stat-value">{receipts.length.toLocaleString()}</span>
                  </div>
                </div>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">↗</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Gross Settled</span>
                    <span className="admin-stat-value">{formatUSDC(grossSettled)}</span>
                  </div>
                </div>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">%</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Protocol Fees ({formatProtocolFee(PROTOCOL_FEE_BPS)})</span>
                    <span className="admin-stat-value">{formatUSDC(protocolFees)}</span>
                  </div>
                </div>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">⊘</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Voided Receipts</span>
                    <span className="admin-stat-value">{voidedCount.toLocaleString()}</span>
                  </div>
                </div>
              </>
            )}
          </div>

          {/* Settlement — replaces the old custodial payout queue */}
          <section className="admin-section">
            <div className="admin-section-header">
              <h2 className="admin-section-title">// SETTLEMENT</h2>
            </div>
            <div style={{ padding: '1rem', fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', lineHeight: 1.6 }}>
              <p style={{ marginTop: 0 }}>
                There is no float to reconcile and no payout queue. Checkout is a single
                atomic wallet-to-wallet transaction: the buyer pays the developer (and,
                for hosting fees, the operator) and the node mints a signed receipt.
                Entitlements are read from those receipts.
              </p>
              <table className="admin-table" aria-label="Settlement summary">
                <tbody>
                  <tr>
                    <td style={{ color: 'var(--color-text-muted)' }}>Held by this node</td>
                    <td className="amount-cell">{formatUSDC(0)}</td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--color-text-muted)' }}>Settled to developers &amp; operators</td>
                    <td className="amount-cell">{formatUSDC(toCounterparties)}</td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--color-text-muted)' }}>Protocol fee</td>
                    <td className="amount-cell">{formatProtocolFee(PROTOCOL_FEE_BPS)}</td>
                  </tr>
                  <tr>
                    <td style={{ color: 'var(--color-text-muted)' }}>Rail key</td>
                    <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-secondary)' }}>
                      {shortKey(receipts.find(r => r.rail)?.rail)}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>

          {/* Receipts */}
          <section className="admin-section">
            <div className="admin-section-header">
              <h2 className="admin-section-title">// RECENT RECEIPTS</h2>
              <div className="finance-filter-row">
                <select
                  value={kindFilter}
                  onChange={(e) => { setKindFilter(e.target.value); setCurrentPage(1); }}
                  aria-label="Filter receipts"
                >
                  <option value="all">All</option>
                  <option value="items">Item Purchases</option>
                  <option value="hosting">Hosting Fees</option>
                  <option value="tiers">Tiers</option>
                </select>
              </div>
            </div>
            {loadingReceipts ? (
              <div className="admin-loading" aria-busy="true" style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>
                <span className="spinner" aria-hidden="true" /> Loading receipts&hellip;
              </div>
            ) : filteredReceipts.length === 0 ? (
              <p style={{ padding: '1rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                No receipts found
              </p>
            ) : (
              <>
                <table className="admin-table" aria-label="Receipts">
                  <thead>
                    <tr>
                      <th>Receipt</th>
                      <th>Kind</th>
                      <th>Buyer</th>
                      <th>Paid To</th>
                      <th>Game</th>
                      <th>Total</th>
                      <th>Protocol Fee</th>
                      <th>Date</th>
                      <th>Status</th>
                      <th>Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paginatedReceipts.map(receipt => (
                      <tr key={receipt.id}>
                        <td className="txn-id">{String(receipt.id).slice(0, 12)}</td>
                        <td>
                          <span className={`type-badge ${receipt.kind}`}>
                            {kindLabel(receipt.kind)}
                          </span>
                        </td>
                        <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-secondary)' }}>
                          {receipt.buyer}
                        </td>
                        <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-2xs)', color: 'var(--color-text-muted)' }}>
                          {shortKey(receipt.payee)}
                        </td>
                        <td style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                          {receipt.game}
                        </td>
                        <td className="amount-cell">{formatUSDC(receipt.total)}</td>
                        <td className="amount-cell">{formatUSDC(receipt.protocolFee)}</td>
                        <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-2xs)', color: 'var(--color-text-muted)' }}>
                          {receipt.date}
                        </td>
                        <td>
                          <span className={`status-badge ${receipt.voided ? 'refunded' : 'completed'}`}>
                            {receipt.voided ? 'voided' : 'valid'}
                          </span>
                        </td>
                        <td>
                          {!receipt.voided && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => { setVoidingReceipt(receipt); setVoidError(null); setVoidSuccess(null); }}
                              aria-label={`Void receipt ${String(receipt.id).slice(0, 12)}`}
                            >
                              Void
                            </Button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div style={{ padding: '0.75rem 1rem', borderTop: '1px solid var(--color-border)' }}>
                  <Pagination
                    total={filteredReceipts.length}
                    perPage={perPage}
                    currentPage={currentPage}
                    onPageChange={setCurrentPage}
                  />
                </div>
              </>
            )}
          </section>
        </main>
      </div>
    </Layout>
  );
}

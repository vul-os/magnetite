import { useState, useEffect, useCallback } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
import { api } from '../../api/client';
import './admin.css';

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8080';

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

/* Mock data — only used when VITE_USE_MOCKS=true */
const MOCK_TRANSACTIONS = import.meta.env.VITE_USE_MOCKS
  ? [
      { id: 'txn_001', type: 'game_fee', user: 'CryptoGamer42',  amount:    2.50, game: 'Cosmic Raiders',    date: '2024-05-19 14:32', status: 'completed' },
      { id: 'txn_002', type: 'game_fee', user: 'NeonRacer99',    amount:    1.50, game: 'Neon Drift',        date: '2024-05-19 14:28', status: 'completed' },
      { id: 'txn_003', type: 'payout',   user: 'PixelMaster',    amount: -500.00, game: 'Galaxy Conquest',   date: '2024-05-19 13:45', status: 'pending'   },
    ]
  : null;

const MOCK_PENDING_PAYOUTS = import.meta.env.VITE_USE_MOCKS
  ? [
      { id: 1, user: 'PixelMaster',    amount: 500.00, games: 'Galaxy Conquest',         requestDate: '2024-05-19', method: 'Wise'   },
      { id: 2, user: 'CryptoGamer42',  amount: 750.00, games: 'Cosmic Raiders',           requestDate: '2024-05-18', method: 'Wise'   },
      { id: 3, user: 'IndieDev_Mike',  amount: 320.00, games: 'Dungeon Realms',           requestDate: '2024-05-17', method: 'Bank Transfer' },
    ]
  : null;

const MOCK_STATS = import.meta.env.VITE_USE_MOCKS
  ? { totalRevenue: 45892.5, monthlyRevenue: 12450.0, platformFees: 6883.88, pendingPayouts: 1570.0 }
  : null;

function normaliseTransaction(t) {
  return {
    id:     t.id,
    type:   t.tx_type ?? t.type ?? 'unknown',
    user:   t.username ?? t.user ?? 'Unknown',
    amount: parseFloat(t.amount ?? 0),
    game:   t.game_title ?? t.game ?? '—',
    date:   t.created_at ? t.created_at.replace('T', ' ').slice(0, 16) : '',
    status: t.status ?? 'unknown',
  };
}

export default function Finance() {
  const [stats, setStats]                       = useState(MOCK_STATS);
  const [transactions, setTransactions]         = useState(MOCK_TRANSACTIONS ?? []);
  const [pendingPayouts, setPendingPayouts]     = useState(MOCK_PENDING_PAYOUTS ?? []);
  const [loadingStats, setLoadingStats]         = useState(!MOCK_STATS);
  const [loadingTxns, setLoadingTxns]           = useState(!MOCK_TRANSACTIONS);
  const [error, setError]                       = useState(null);
  const [transactionFilter, setTransactionFilter] = useState('all');
  const [currentPage, setCurrentPage]             = useState(1);
  const [processingPayout, setProcessingPayout]   = useState(null);
  const [payoutError, setPayoutError]             = useState(null);
  const [refundingTxn, setRefundingTxn]           = useState(null);   // txn being refunded
  const [refundReason, setRefundReason]           = useState('');
  const [refundError, setRefundError]             = useState(null);
  const [refundSuccess, setRefundSuccess]         = useState(null);
  const perPage = 10;

  const fetchData = useCallback(async () => {
    if (import.meta.env.VITE_USE_MOCKS) return;

    setLoadingStats(true);
    setLoadingTxns(true);
    setError(null);

    try {
      const [revenueRes, txnRes] = await Promise.all([
        authFetch('/api/admin/revenue'),
        authFetch('/api/admin/transactions?limit=100'),
      ]);

      if (revenueRes.ok) {
        const d = await revenueRes.json();
        setStats({
          totalRevenue:   parseFloat(d.total_game_revenue ?? d.total_platform_revenue ?? 0),
          monthlyRevenue: parseFloat(d.total_platform_revenue ?? 0),
          platformFees:   parseFloat(d.total_platform_revenue ?? 0),
          pendingPayouts: parseFloat(d.pending_payouts ?? 0),
        });
      }

      if (txnRes.ok) {
        const d = await txnRes.json();
        const raw = d.data ?? d ?? [];
        const normalised = Array.isArray(raw) ? raw.map(normaliseTransaction) : [];
        setTransactions(normalised);
        /* pending payouts are payout-type txns with status pending */
        setPendingPayouts(
          normalised
            .filter(t => (t.type === 'payout' || t.type === 'withdrawal') && t.status === 'pending')
            .map((t, i) => ({
              id:          t.id,
              user:        t.user,
              amount:      Math.abs(t.amount),
              games:       t.game,
              requestDate: t.date.split(' ')[0],
              method:      'Wise',
              _idx:        i,
            }))
        );
      }
    } catch (err) {
      setError(err.message || 'Failed to load finance data');
    } finally {
      setLoadingStats(false);
      setLoadingTxns(false);
    }
  }, []);

  // Fetch finance data from the admin API (external system) on mount.
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { fetchData(); }, [fetchData]);

  const filteredTransactions = transactions.filter(txn => {
    if (transactionFilter === 'payouts')   return txn.type === 'payout' || txn.type === 'withdrawal';
    if (transactionFilter === 'game_fees') return txn.type === 'game_fee' || txn.type === 'fee';
    return true;
  });

  const paginatedTransactions = filteredTransactions.slice(
    (currentPage - 1) * perPage,
    currentPage * perPage
  );

  const handleProcessPayout = async (payoutId) => {
    setProcessingPayout(payoutId);
    setPayoutError(null);
    try {
      const res = await authFetch(`/api/admin/payouts/${payoutId}/process`, { method: 'POST' });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || `Failed to process payout (HTTP ${res.status})`);
      }
      setPendingPayouts(prev => prev.filter(p => p.id !== payoutId));
    } catch (err) {
      setPayoutError(err.message);
    } finally {
      setProcessingPayout(null);
    }
  };

  const handleProcessAll = async () => {
    setProcessingPayout('all');
    setPayoutError(null);
    const errors = [];
    for (const payout of pendingPayouts) {
      try {
        const res = await authFetch(`/api/admin/payouts/${payout.id}/process`, { method: 'POST' });
        if (!res.ok) {
          const err = await res.json().catch(() => ({}));
          errors.push(err.message || `Failed for ${payout.user}`);
        } else {
          setPendingPayouts(prev => prev.filter(p => p.id !== payout.id));
        }
      } catch (err) {
        errors.push(err.message);
      }
    }
    if (errors.length > 0) {
      setPayoutError(errors.join('; '));
    }
    setProcessingPayout(null);
  };

  const handleRefundConfirm = async () => {
    if (!refundingTxn) return;
    setRefundError(null);
    try {
      await api.admin.refundTransaction(refundingTxn.id, { reason: refundReason || undefined });
      setRefundSuccess(`Refund initiated for transaction ${String(refundingTxn.id).slice(0, 12)}`);
      setRefundingTxn(null);
      setRefundReason('');
      // Mark the transaction as refunded in local state
      setTransactions(prev =>
        prev.map(t => t.id === refundingTxn.id ? { ...t, status: 'refunded' } : t)
      );
    } catch (err) {
      setRefundError(err.message || 'Refund failed');
    }
  };

  const displayStats = stats || { totalRevenue: 0, monthlyRevenue: 0, platformFees: 0, pendingPayouts: 0 };

  return (
    <Layout>
      <div className="admin-layout">
        <AdminSidebar />
        <main className="admin-main reveal">
          <header className="admin-header reveal-1">
            <div>
              <span className="kicker">// Platform Control</span>
              <h1>Finance Dashboard</h1>
              <p>Revenue, fees, and payout management</p>
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

          {payoutError && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              {payoutError}
            </div>
          )}

          {refundError && (
            <div className="admin-error-banner" role="alert">
              <span className="auth-error-icon" aria-hidden="true">!</span>
              Refund error: {refundError}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={() => setRefundError(null)}>
                Dismiss
              </button>
            </div>
          )}

          {refundSuccess && (
            <div className="admin-success-banner" role="status" style={{ padding: '0.75rem 1rem', marginBottom: '1rem', borderRadius: 'var(--radius)', background: 'rgba(34,197,94,0.1)', border: '1px solid rgba(34,197,94,0.3)', color: 'var(--color-success, #22c55e)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
              {refundSuccess}
              <button className="settings-action-btn" style={{ marginLeft: '1rem' }} onClick={() => setRefundSuccess(null)}>
                Dismiss
              </button>
            </div>
          )}

          {/* Refund confirmation modal */}
          {refundingTxn && (
            <div
              role="dialog"
              aria-modal="true"
              aria-labelledby="refund-dialog-title"
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
                <h2 id="refund-dialog-title" style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-base)', marginBottom: '0.5rem', color: 'var(--color-text-primary)' }}>
                  // CONFIRM REFUND
                </h2>
                <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', marginBottom: '1.25rem' }}>
                  Refund transaction <code style={{ fontFamily: 'var(--font-mono)', color: 'var(--color-accent)' }}>{String(refundingTxn.id).slice(0, 12)}</code> of <strong>${Math.abs(refundingTxn.amount).toFixed(2)}</strong> for user <strong>{refundingTxn.user}</strong>?
                </p>
                <label style={{ display: 'block', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)', marginBottom: '0.4rem' }}>
                  Reason (optional)
                </label>
                <textarea
                  value={refundReason}
                  onChange={e => setRefundReason(e.target.value)}
                  placeholder="e.g. duplicate charge, user request"
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
                    onClick={() => { setRefundingTxn(null); setRefundReason(''); setRefundError(null); }}
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={handleRefundConfirm}
                  >
                    Confirm Refund
                  </Button>
                </div>
              </div>
            </div>
          )}

          <div className="admin-stats-grid">
            {loadingStats ? (
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
                  <div className="admin-stat-icon">$</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Total Revenue</span>
                    <span className="admin-stat-value">${displayStats.totalRevenue.toLocaleString()}</span>
                  </div>
                </div>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">↗</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Platform Revenue</span>
                    <span className="admin-stat-value">${displayStats.monthlyRevenue.toLocaleString()}</span>
                  </div>
                </div>
                <div className="admin-stat-card">
                  <div className="admin-stat-icon">%</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Platform Fees (15%)</span>
                    <span className="admin-stat-value">${displayStats.platformFees.toLocaleString()}</span>
                  </div>
                </div>
                <div className="admin-stat-card warning">
                  <div className="admin-stat-icon">⏳</div>
                  <div className="admin-stat-info">
                    <span className="admin-stat-label">Pending Payouts</span>
                    <span className="admin-stat-value">${displayStats.pendingPayouts.toLocaleString()}</span>
                  </div>
                </div>
              </>
            )}
          </div>

          {/* Pending payouts */}
          <section className="admin-section">
            <div className="admin-section-header">
              <h2 className="admin-section-title">// PENDING PAYOUTS</h2>
              {pendingPayouts.length > 0 && (
                <Button
                  variant="primary"
                  size="sm"
                  loading={processingPayout === 'all'}
                  onClick={handleProcessAll}
                >
                  Process All
                </Button>
              )}
            </div>
            {pendingPayouts.length === 0 ? (
              <p style={{ padding: '1rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                No pending payouts
              </p>
            ) : (
              <table className="admin-table" aria-label="Pending payouts">
                <thead>
                  <tr>
                    <th>User</th>
                    <th>Amount</th>
                    <th>Games</th>
                    <th>Requested</th>
                    <th>Method</th>
                    <th>Action</th>
                  </tr>
                </thead>
                <tbody>
                  {pendingPayouts.map(payout => (
                    <tr key={payout.id}>
                      <td style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>{payout.user}</td>
                      <td className="amount-cell">${payout.amount.toLocaleString()}</td>
                      <td className="games-cell">{payout.games}</td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-2xs)', color: 'var(--color-text-muted)' }}>
                        {payout.requestDate}
                      </td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-secondary)' }}>
                        {payout.method}
                      </td>
                      <td>
                        <Button
                          variant="primary"
                          size="sm"
                          loading={processingPayout === payout.id}
                          onClick={() => handleProcessPayout(payout.id)}
                        >
                          Process
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          {/* Transactions */}
          <section className="admin-section">
            <div className="admin-section-header">
              <h2 className="admin-section-title">// RECENT TRANSACTIONS</h2>
              <div className="finance-filter-row">
                <select
                  value={transactionFilter}
                  onChange={(e) => { setTransactionFilter(e.target.value); setCurrentPage(1); }}
                  aria-label="Filter transactions"
                >
                  <option value="all">All</option>
                  <option value="game_fees">Game Fees</option>
                  <option value="payouts">Payouts</option>
                </select>
              </div>
            </div>
            {loadingTxns ? (
              <div className="admin-loading" aria-busy="true" style={{ padding: '2rem', textAlign: 'center', color: 'var(--color-text-muted)' }}>
                <span className="spinner" aria-hidden="true" /> Loading transactions&hellip;
              </div>
            ) : filteredTransactions.length === 0 ? (
              <p style={{ padding: '1rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)', fontSize: 'var(--text-sm)' }}>
                No transactions found
              </p>
            ) : (
              <>
                <table className="admin-table" aria-label="Transactions">
                  <thead>
                    <tr>
                      <th>ID</th>
                      <th>Type</th>
                      <th>User</th>
                      <th>Game</th>
                      <th>Amount</th>
                      <th>Date</th>
                      <th>Status</th>
                      <th>Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paginatedTransactions.map(txn => (
                      <tr key={txn.id}>
                        <td className="txn-id">{String(txn.id).slice(0, 12)}</td>
                        <td>
                          <span className={`type-badge ${txn.type}`}>
                            {txn.type === 'game_fee' || txn.type === 'fee' ? 'Fee' : 'Payout'}
                          </span>
                        </td>
                        <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-xs)', color: 'var(--color-text-secondary)' }}>
                          {txn.user}
                        </td>
                        <td style={{ fontSize: 'var(--text-xs)', color: 'var(--color-text-muted)' }}>
                          {txn.game}
                        </td>
                        <td className={`amount-cell${txn.amount < 0 ? ' negative' : ''}`}>
                          {txn.amount < 0 ? '-' : '+'}${Math.abs(txn.amount).toFixed(2)}
                        </td>
                        <td style={{ fontFamily: 'var(--font-mono)', fontSize: 'var(--text-2xs)', color: 'var(--color-text-muted)' }}>
                          {txn.date}
                        </td>
                        <td>
                          <span className={`status-badge ${txn.status}`}>{txn.status}</span>
                        </td>
                        <td>
                          {txn.status !== 'refunded' && txn.amount > 0 && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => { setRefundingTxn(txn); setRefundError(null); setRefundSuccess(null); }}
                              aria-label={`Refund transaction ${String(txn.id).slice(0, 12)}`}
                            >
                              Refund
                            </Button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div style={{ padding: '0.75rem 1rem', borderTop: '1px solid var(--color-border)' }}>
                  <Pagination
                    total={filteredTransactions.length}
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

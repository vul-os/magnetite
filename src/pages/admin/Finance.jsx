import { useState } from 'react';
import Layout from '../../components/Layout';
import AdminSidebar from '../../components/admin/AdminSidebar';
import Pagination from '../../components/Pagination';
import Button from '../../components/common/Button';
import './admin.css';

const MOCK_TRANSACTIONS = [
  { id: 'txn_001', type: 'game_fee', user: 'CryptoGamer42',  amount:   2.50, game: 'Cosmic Raiders',     date: '2024-05-19 14:32', status: 'completed' },
  { id: 'txn_002', type: 'game_fee', user: 'NeonRacer99',    amount:   1.50, game: 'Neon Drift',         date: '2024-05-19 14:28', status: 'completed' },
  { id: 'txn_003', type: 'payout',   user: 'PixelMaster',    amount: -500.00,game: 'Galaxy Conquest',    date: '2024-05-19 13:45', status: 'pending'   },
  { id: 'txn_004', type: 'game_fee', user: 'ProStreamer',     amount:   3.00, game: 'Chess Champions',    date: '2024-05-19 12:15', status: 'completed' },
  { id: 'txn_005', type: 'game_fee', user: 'StarForge_Admin', amount:  5.00, game: 'Fantasy RPG',        date: '2024-05-19 11:50', status: 'completed' },
  { id: 'txn_006', type: 'payout',   user: 'IndieDev_Mike',  amount: -250.00,game: 'Dungeon Realms',     date: '2024-05-19 10:30', status: 'completed' },
  { id: 'txn_007', type: 'game_fee', user: 'GameLover99',    amount:   1.00, game: 'Cosmic Raiders',     date: '2024-05-19 09:45', status: 'completed' },
  { id: 'txn_008', type: 'payout',   user: 'CryptoGamer42',  amount: -750.00,game: 'Cosmic Raiders',     date: '2024-05-18 16:20', status: 'pending'   },
  { id: 'txn_009', type: 'game_fee', user: 'BlockchainGamer', amount:  2.00, game: 'Space Battle',       date: '2024-05-18 15:10', status: 'completed' },
  { id: 'txn_010', type: 'game_fee', user: 'PixelMaster',    amount:   4.50, game: 'Multiplayer Arena',  date: '2024-05-18 14:55', status: 'completed' },
  { id: 'txn_011', type: 'payout',   user: 'StarForge_Admin',amount: -1200.00,game: 'Fantasy RPG',       date: '2024-05-18 12:00', status: 'completed' },
  { id: 'txn_012', type: 'game_fee', user: 'CasualPlayer',   amount:   0.50, game: 'Racing Pro',         date: '2024-05-18 10:30', status: 'completed' },
];

const MOCK_PENDING_PAYOUTS = [
  { id: 1, user: 'PixelMaster',    amount: 500.00, games: 'Galaxy Conquest, Multiplayer Arena', requestDate: '2024-05-19', method: 'USDC Wallet'   },
  { id: 2, user: 'CryptoGamer42', amount: 750.00, games: 'Cosmic Raiders, Puzzle Master',       requestDate: '2024-05-18', method: 'USDC Wallet'   },
  { id: 3, user: 'IndieDev_Mike', amount: 320.00, games: 'Dungeon Realms',                      requestDate: '2024-05-17', method: 'Bank Transfer' },
];

const STATS = {
  totalRevenue:   45892.50,
  monthlyRevenue: 12450.00,
  platformFees:   6883.88,
  pendingPayouts: 1570.00,
};

export default function Finance() {
  const [transactionFilter, setTransactionFilter] = useState('all');
  const [currentPage, setCurrentPage]             = useState(1);
  const [processingPayout, setProcessingPayout]   = useState(null);
  const perPage = 10;

  const filteredTransactions = MOCK_TRANSACTIONS.filter(txn => {
    if (transactionFilter === 'payouts')   return txn.type === 'payout';
    if (transactionFilter === 'game_fees') return txn.type === 'game_fee';
    return true;
  });

  const paginatedTransactions = filteredTransactions.slice(
    (currentPage - 1) * perPage,
    currentPage * perPage
  );

  const handleProcessPayout = async (payoutId) => {
    setProcessingPayout(payoutId);
    await new Promise(r => setTimeout(r, 1000));
    setProcessingPayout(null);
  };

  const handleProcessAll = async () => {
    setProcessingPayout('all');
    await new Promise(r => setTimeout(r, 1500));
    setProcessingPayout(null);
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
              <p>Revenue, fees, and payout management</p>
            </div>
          </header>

          <div className="admin-stats-grid">
            <div className="admin-stat-card">
              <div className="admin-stat-icon">$</div>
              <div className="admin-stat-info">
                <span className="admin-stat-label">Total Revenue</span>
                <span className="admin-stat-value">${STATS.totalRevenue.toLocaleString()}</span>
              </div>
            </div>
            <div className="admin-stat-card">
              <div className="admin-stat-icon">↗</div>
              <div className="admin-stat-info">
                <span className="admin-stat-label">Monthly Revenue</span>
                <span className="admin-stat-value">${STATS.monthlyRevenue.toLocaleString()}</span>
              </div>
            </div>
            <div className="admin-stat-card">
              <div className="admin-stat-icon">%</div>
              <div className="admin-stat-info">
                <span className="admin-stat-label">Platform Fees (15%)</span>
                <span className="admin-stat-value">${STATS.platformFees.toLocaleString()}</span>
              </div>
            </div>
            <div className="admin-stat-card warning">
              <div className="admin-stat-icon">⏳</div>
              <div className="admin-stat-info">
                <span className="admin-stat-label">Pending Payouts</span>
                <span className="admin-stat-value">${STATS.pendingPayouts.toLocaleString()}</span>
              </div>
            </div>
          </div>

          {/* Pending payouts */}
          <section className="admin-section">
            <div className="admin-section-header">
              <h2 className="admin-section-title">// PENDING PAYOUTS</h2>
              <Button
                variant="primary"
                size="sm"
                loading={processingPayout === 'all'}
                onClick={handleProcessAll}
              >
                Process All
              </Button>
            </div>
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
                {MOCK_PENDING_PAYOUTS.map(payout => (
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
                </tr>
              </thead>
              <tbody>
                {paginatedTransactions.map(txn => (
                  <tr key={txn.id}>
                    <td className="txn-id">{txn.id}</td>
                    <td>
                      <span className={`type-badge ${txn.type}`}>
                        {txn.type === 'game_fee' ? 'Fee' : 'Payout'}
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
          </section>
        </main>
      </div>
    </Layout>
  );
}

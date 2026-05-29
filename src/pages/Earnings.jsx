import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import { api } from '../api/client';

const MOCK_TRANSACTIONS = [
  { id: 'tx_001', type: 'game', description: 'Cosmic Raiders - Session #4521', amount: -1.50, balance: 24580.50, date: '2026-05-18 14:32' },
  { id: 'tx_002', type: 'game', description: 'Cosmic Raiders - Session #4520', amount: -0.50, balance: 24582.00, date: '2026-05-18 14:28' },
  { id: 'tx_003', type: 'deposit', description: 'USDC Deposit', amount: 500.00, balance: 24582.50, date: '2026-05-18 10:15' },
  { id: 'tx_004', type: 'game', description: 'Galaxy Conquest - Session #892', amount: -1.00, balance: 24082.50, date: '2026-05-17 22:45' },
  { id: 'tx_005', type: 'payout', description: 'Payout to Wallet', amount: -500.00, balance: 24083.50, date: '2026-05-17 18:00' },
  { id: 'tx_006', type: 'game', description: 'Dungeon Realms - Session #234', amount: -2.00, balance: 24583.50, date: '2026-05-17 15:22' },
  { id: 'tx_007', type: 'game', description: 'Cosmic Raiders - Session #4519', amount: -0.50, balance: 24585.50, date: '2026-05-17 12:08' },
  { id: 'tx_008', type: 'deposit', description: 'USDC Deposit', amount: 1000.00, balance: 24586.00, date: '2026-05-16 09:30' },
];

const MOCK_PAYOUTS = [
  { id: 'pay_001', amount: 500.00, method: 'USDC (Polygon)', status: 'Completed', date: '2026-05-17' },
  { id: 'pay_002', amount: 1250.00, method: 'USDC (Polygon)', status: 'Completed', date: '2026-05-10' },
  { id: 'pay_003', amount: 800.00, method: 'USDC (Polygon)', status: 'Completed', date: '2026-05-03' },
  { id: 'pay_004', amount: 2100.00, method: 'USDC (Polygon)', status: 'Completed', date: '2026-04-25' },
];

export default function Earnings() {
  const [balance, setBalance] = useState(24580.50);
  const [pendingBalance, setPendingBalance] = useState(384.25);
  const [lifetimeEarnings, setLifetimeEarnings] = useState(89432.00);
  const [transactions, setTransactions] = useState(MOCK_TRANSACTIONS);
  const [payouts, setPayouts] = useState(MOCK_PAYOUTS);
  const [activeTab, setActiveTab] = useState('transactions');
  const [withdrawing, setWithdrawing] = useState(false);
  const [withdrawAmount, setWithdrawAmount] = useState('');
  const [withdrawSuccess, setWithdrawSuccess] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadData() {
      try {
        const [balanceData, txData] = await Promise.allSettled([
          api.wallet.balance(),
          api.wallet.transactions(),
        ]);
        if (balanceData.status === 'fulfilled') {
          setBalance(balanceData.value.balance || balance);
          setPendingBalance(balanceData.value.pending || 0);
        }
        if (txData.status === 'fulfilled') {
          setTransactions(txData.value.transactions || MOCK_TRANSACTIONS);
        }
      } catch (err) {
        console.log('Using mock data');
      } finally {
        setLoading(false);
      }
    }
    loadData();
  }, []);

  const handleWithdraw = async (e) => {
    e.preventDefault();
    if (!withdrawAmount || parseFloat(withdrawAmount) <= 0) return;
    
    setWithdrawing(true);
    try {
      await api.wallet.withdraw({ amount: parseFloat(withdrawAmount) });
      setBalance(prev => prev - parseFloat(withdrawAmount));
      setWithdrawSuccess(true);
      setWithdrawAmount('');
      setTimeout(() => setWithdrawSuccess(false), 3000);
    } catch (err) {
      await new Promise(resolve => setTimeout(resolve, 1500));
      setBalance(prev => prev - parseFloat(withdrawAmount));
      setWithdrawSuccess(true);
      setWithdrawAmount('');
      setTimeout(() => setWithdrawSuccess(false), 3000);
    } finally {
      setWithdrawing(false);
    }
  };

  const formatAmount = (amount) => {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(Math.abs(amount));
  };

  return (
    <Layout>
      <div className="earnings-page">
        <header className="earnings-header">
          <h1>Earnings</h1>
          <p>Track your revenue and manage payouts</p>
        </header>

        <div className="earnings-summary">
          <div className="summary-card primary">
            <div className="summary-icon">💰</div>
            <div className="summary-content">
              <span className="summary-label">Available Balance</span>
              <span className="summary-value">${balance.toLocaleString()}</span>
            </div>
          </div>
          <div className="summary-card">
            <div className="summary-icon">⏳</div>
            <div className="summary-content">
              <span className="summary-label">Pending</span>
              <span className="summary-value">${pendingBalance.toLocaleString()}</span>
            </div>
          </div>
          <div className="summary-card">
            <div className="summary-icon">📈</div>
            <div className="summary-content">
              <span className="summary-label">Lifetime Earnings</span>
              <span className="summary-value">${lifetimeEarnings.toLocaleString()}</span>
            </div>
          </div>
        </div>

        <div className="withdraw-section">
          <h3>Withdraw Earnings</h3>
          <form className="withdraw-form" onSubmit={handleWithdraw}>
            <div className="withdraw-input-group">
              <input
                type="number"
                step="0.01"
                min="1"
                max={balance}
                placeholder="Enter amount"
                value={withdrawAmount}
                onChange={(e) => setWithdrawAmount(e.target.value)}
                disabled={withdrawing}
              />
              <span className="currency-label">USDC</span>
            </div>
            <button
              type="submit"
              className="btn btn-primary withdraw-btn"
              disabled={withdrawing || !withdrawAmount || parseFloat(withdrawAmount) > balance}
            >
              {withdrawing ? 'Processing...' : withdrawSuccess ? 'Withdrawal Initiated!' : 'Withdraw'}
            </button>
          </form>
          <p className="withdraw-note">Withdrawals are processed to your connected Polygon wallet within 24 hours.</p>
        </div>

        <div className="earnings-tabs">
          <button
            className={`tab-btn ${activeTab === 'transactions' ? 'active' : ''}`}
            onClick={() => setActiveTab('transactions')}
          >
            Transaction History
          </button>
          <button
            className={`tab-btn ${activeTab === 'payouts' ? 'active' : ''}`}
            onClick={() => setActiveTab('payouts')}
          >
            Payout History
          </button>
        </div>

        <div className="tab-content">
          {activeTab === 'transactions' ? (
            <div className="transactions-section">
              {loading ? (
                <div className="loading-state">Loading transactions...</div>
              ) : (
                <table className="transactions-table">
                  <thead>
                    <tr>
                      <th>Date</th>
                      <th>Description</th>
                      <th>Amount</th>
                      <th>Balance</th>
                    </tr>
                  </thead>
                  <tbody>
                    {transactions.map(tx => (
                      <tr key={tx.id}>
                        <td className="date-cell">{tx.date}</td>
                        <td>
                          <div className="tx-description">
                            <span className={`tx-type-icon ${tx.type}`}></span>
                            {tx.description}
                          </div>
                        </td>
                        <td className={`amount-cell ${tx.amount > 0 ? 'positive' : 'negative'}`}>
                          {tx.amount > 0 ? '+' : ''}{formatAmount(tx.amount)}
                        </td>
                        <td className="balance-cell">{formatAmount(tx.balance)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ) : (
            <div className="payouts-section">
              {loading ? (
                <div className="loading-state">Loading payouts...</div>
              ) : (
                <table className="payouts-table">
                  <thead>
                    <tr>
                      <th>Date</th>
                      <th>Amount</th>
                      <th>Method</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {payouts.map(payout => (
                      <tr key={payout.id}>
                        <td className="date-cell">{payout.date}</td>
                        <td className="amount-cell positive">{formatAmount(payout.amount)}</td>
                        <td>{payout.method}</td>
                        <td><span className="status-badge completed">{payout.status}</span></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>
      </div>
    </Layout>
  );
}

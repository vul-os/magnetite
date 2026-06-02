import { useState } from 'react';
import Modal from './Modal';
import DepositForm from './DepositForm';
import WithdrawForm from './WithdrawForm';

export default function PaymentModal({ isOpen, onClose }) {
  const [activeTab, setActiveTab] = useState('deposit');

  const handleSuccess = (_result) => {
    // Intentionally silent — payment result may contain sensitive data.
  };

  const handleError = (_error) => {
    // Errors are surfaced to the user by DepositForm / WithdrawForm directly.
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={activeTab === 'deposit' ? 'Add Funds (USD)' : 'Request Payout (USD)'}
      size="md"
    >
      <div className="payment-modal-tabs">
        <button
          className={`payment-tab ${activeTab === 'deposit' ? 'active' : ''}`}
          onClick={() => setActiveTab('deposit')}
        >
          Deposit
        </button>
        <button
          className={`payment-tab ${activeTab === 'withdraw' ? 'active' : ''}`}
          onClick={() => setActiveTab('withdraw')}
        >
          Withdraw
        </button>
      </div>

      <div className="payment-modal-content">
        {activeTab === 'deposit' ? (
          <DepositForm onSuccess={handleSuccess} onError={handleError} />
        ) : (
          <WithdrawForm onSuccess={handleSuccess} onError={handleError} />
        )}
      </div>
    </Modal>
  );
}

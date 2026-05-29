import { useState } from 'react';
import Modal from './Modal';
import DepositForm from './DepositForm';
import WithdrawForm from './WithdrawForm';

export default function PaymentModal({ isOpen, onClose }) {
  const [activeTab, setActiveTab] = useState('deposit');

  const handleSuccess = (result) => {
    console.log('Payment success:', result);
  };

  const handleError = (error) => {
    console.error('Payment error:', error);
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={activeTab === 'deposit' ? 'Deposit USDC' : 'Withdraw USDC'}
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

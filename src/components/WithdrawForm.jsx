import { useState } from 'react';
import Input from './common/Input';
import Button from './common/Button';

const NETWORKS = [
  { id: 'ethereum', name: 'Ethereum (ERC-20)', symbol: 'ETH' },
  { id: 'polygon', name: 'Polygon', symbol: 'MATIC' },
  { id: 'bsc', name: 'BNB Smart Chain (BEP-20)', symbol: 'BNB' },
];

export default function WithdrawForm({ onSuccess, onError }) {
  const [amount, setAmount] = useState('');
  const [destinationAddress, setDestinationAddress] = useState('');
  const [network, setNetwork] = useState('ethereum');
  const [isProcessing, setIsProcessing] = useState(false);
  const [status, setStatus] = useState(null);
  const [errors, setErrors] = useState({});
  const [reference, setReference] = useState('');

  const validateForm = () => {
    const newErrors = {};

    if (!amount || parseFloat(amount) <= 0) {
      newErrors.amount = 'Please enter a valid amount';
    }

    if (!destinationAddress) {
      newErrors.destinationAddress = 'Destination address is required';
    } else if (!/^0x[a-fA-F0-9]{40}$/.test(destinationAddress) && !/^bnb1[a-zA-Z0-9]{38}$/.test(destinationAddress)) {
      newErrors.destinationAddress = 'Invalid wallet address format';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e) => {
    e.preventDefault();

    if (!validateForm()) return;

    setIsProcessing(true);
    setStatus(null);

    try {
      await new Promise((resolve, reject) => {
        setTimeout(() => {
          if (Math.random() > 0.9) {
            reject(new Error('Network congestion. Please try again.'));
          } else {
            resolve();
          }
        }, 1500);
      });

      const txReference = `WD-${Date.now()}`;
      setReference(txReference);
      setStatus('success');
      onSuccess?.({ reference: txReference, amount: parseFloat(amount), network });
      setAmount('');
      setDestinationAddress('');
      setTimeout(() => {
        setStatus(null);
        setReference('');
      }, 3000);
    } catch (err) {
      setStatus('error');
      setErrors({ submit: err.message });
      onError?.(err);
      setTimeout(() => setStatus(null), 3000);
    }

    setIsProcessing(false);
  };

  return (
    <form onSubmit={handleSubmit} className="withdraw-form">
      <Input
        label="Amount (USDC)"
        type="number"
        placeholder="0.00"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
        error={errors.amount}
        min="0"
        step="0.01"
      />

      <div className="network-selector">
        <label className="input-label">Network</label>
        <div className="network-options">
          {NETWORKS.map((net) => (
            <button
              key={net.id}
              type="button"
              className={`network-option ${network === net.id ? 'active' : ''}`}
              onClick={() => setNetwork(net.id)}
            >
              <span className="network-symbol">{net.symbol}</span>
              <span className="network-name">{net.name}</span>
            </button>
          ))}
        </div>
      </div>

      <Input
        label="Destination Wallet Address"
        placeholder="0x..."
        value={destinationAddress}
        onChange={(e) => setDestinationAddress(e.target.value)}
        error={errors.destinationAddress}
      />

      {errors.submit && (
        <div className="form-status error">{errors.submit}</div>
      )}

      {status === 'success' && (
        <div className="form-status success">
          Withdrawal initiated successfully! Reference: {reference}
        </div>
      )}

      <Button
        type="submit"
        variant="primary"
        size="lg"
        loading={isProcessing}
        disabled={!amount || !destinationAddress || parseFloat(amount) <= 0}
        className="withdraw-submit-btn"
      >
        {isProcessing ? 'Processing...' : 'Withdraw USDC'}
      </Button>

      <p className="withdraw-disclaimer">
        Withdrawals are processed within 24-48 hours. A small network fee may apply.
      </p>
    </form>
  );
}

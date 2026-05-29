import { useState } from 'react';
import Input from './common/Input';
import Button from './common/Button';
import { api } from '../api/client';

const PAYSTACK_PUBLIC_KEY = import.meta.env.VITE_PAYSTACK_PUBLIC_KEY;

const SUBSCRIPTION_PLANS = [
  { id: 'basic', name: 'Basic', price: 4.99, hours: 10 },
  { id: 'pro', name: 'Pro', price: 9.99, hours: 50 },
  { id: 'unlimited', name: 'Unlimited', price: 19.99, hours: 'Unlimited' },
];

export default function DepositForm({ onSuccess, onError }) {
  const [amount, setAmount] = useState('');
  const [paymentMethod, setPaymentMethod] = useState('paystack');
  const [walletAddress] = useState('0x1234...5678');
  const [isProcessing, setIsProcessing] = useState(false);
  const [status, setStatus] = useState(null);
  const [mode, setMode] = useState('deposit');

  const handlePaystackPayment = async () => {
    if (!amount || parseFloat(amount) <= 0) return;

    setIsProcessing(true);
    try {
      const handler = window.PaystackPop && window.PaystackPop.setup({
        key: PAYSTACK_PUBLIC_KEY,
        amount: parseFloat(amount) * 100,
        currency: 'ZAR',
        email: 'user@example.com',
        callback: (response) => {
          setStatus('success');
          onSuccess?.(response);
          setTimeout(() => setStatus(null), 3000);
        },
        onClose: () => {
          setIsProcessing(false);
        },
      });
      if (handler) handler.openIframe();
    } catch (err) {
      setStatus('error');
      onError?.(err);
      setTimeout(() => setStatus(null), 3000);
    }
    setIsProcessing(false);
  };

  const handleDirectTransfer = async () => {
    if (!amount || parseFloat(amount) <= 0) return;

    setIsProcessing(true);
    try {
      await new Promise((resolve) => setTimeout(resolve, 1500));
      setStatus('success');
      onSuccess?.({ reference: `TX-${Date.now()}` });
      setAmount('');
      setTimeout(() => setStatus(null), 3000);
    } catch (err) {
      setStatus('error');
      onError?.(err);
    }
    setIsProcessing(false);
  };

  const handleSubscribe = async (planId) => {
    setIsProcessing(true);
    try {
      await api.subscriptions.create({ plan_id: planId });
      setStatus('success');
      onSuccess?.({ plan_id: planId });
      setTimeout(() => setStatus(null), 3000);
    } catch (err) {
      setStatus('error');
      onError?.(err);
      setTimeout(() => setStatus(null), 3000);
    }
    setIsProcessing(false);
  };

  const handleSubmit = (e) => {
    e.preventDefault();
    if (paymentMethod === 'paystack') {
      handlePaystackPayment();
    } else {
      handleDirectTransfer();
    }
  };

  return (
    <div className="deposit-form-container">
      <div className="form-mode-tabs">
        <button
          className={`mode-tab ${mode === 'deposit' ? 'active' : ''}`}
          onClick={() => setMode('deposit')}
        >
          Add Funds
        </button>
        <button
          className={`mode-tab ${mode === 'subscribe' ? 'active' : ''}`}
          onClick={() => setMode('subscribe')}
        >
          Subscribe
        </button>
      </div>

      {mode === 'deposit' ? (
        <form onSubmit={handleSubmit} className="deposit-form">
          <Input
            label="Amount (USDC)"
            type="number"
            placeholder="0.00"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            min="0"
            step="0.01"
          />

          <div className="payment-methods">
            <label className="payment-method-label">Payment Method</label>
            <div className="payment-method-options">
              <button
                type="button"
                className={`payment-method-option ${paymentMethod === 'paystack' ? 'active' : ''}`}
                onClick={() => setPaymentMethod('paystack')}
              >
                <span className="payment-method-icon">₦</span>
                <span>Paystack (ZAR)</span>
              </button>
              <button
                type="button"
                className={`payment-method-option ${paymentMethod === 'transfer' ? 'active' : ''}`}
                onClick={() => setPaymentMethod('transfer')}
              >
                <span className="payment-method-icon">◎</span>
                <span>Direct Transfer</span>
              </button>
            </div>
          </div>

          {paymentMethod === 'transfer' && (
            <div className="wallet-address-display">
              <label className="input-label">USDC Wallet Address</label>
              <div className="wallet-address-box">
                <code>{walletAddress}</code>
                <button
                  type="button"
                  className="copy-button"
                  onClick={() => navigator.clipboard.writeText(walletAddress)}
                >
                  Copy
                </button>
              </div>
              <span className="input-helper">Send USDC to this address. Deposits are processed within 24 hours.</span>
            </div>
          )}

          {status === 'success' && (
            <div className="form-status success">
              {paymentMethod === 'paystack' ? 'Payment initiated! Check your email for confirmation.' : 'Transfer initiated! Your balance will be updated upon confirmation.'}
            </div>
          )}

          {status === 'error' && (
            <div className="form-status error">
              Payment failed. Please try again or contact support.
            </div>
          )}

          <Button
            type="submit"
            variant="primary"
            size="lg"
            loading={isProcessing}
            disabled={!amount || parseFloat(amount) <= 0}
            className="deposit-submit-btn"
          >
            {isProcessing ? 'Processing...' : paymentMethod === 'paystack' ? `Pay ${amount || '0'} ZAR` : 'Confirm Transfer'}
          </Button>
        </form>
      ) : (
        <div className="subscription-form">
          <div className="subscription-plans">
            {SUBSCRIPTION_PLANS.map((plan) => (
              <div key={plan.id} className="subscription-plan-option">
                <div className="plan-info">
                  <span className="plan-name">{plan.name}</span>
                  <span className="plan-price">${plan.price}/mo</span>
                  <span className="plan-hours">{plan.hours} hours/month</span>
                </div>
                <Button
                  variant="primary"
                  size="md"
                  loading={isProcessing}
                  onClick={() => handleSubscribe(plan.id)}
                >
                  Subscribe
                </Button>
              </div>
            ))}
          </div>

          <p className="subscription-note">
            Subscriptions renew monthly. Cancel anytime from your wallet settings.
          </p>

          {status === 'success' && (
            <div className="form-status success">
              Subscription activated! You now have access to premium games.
            </div>
          )}

          {status === 'error' && (
            <div className="form-status error">
              Subscription failed. Please try again or contact support.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
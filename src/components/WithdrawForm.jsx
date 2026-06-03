import { useState, useEffect } from 'react';
import Input from './common/Input';
import Button from './common/Button';
import { api } from '../api/client';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

export default function WithdrawForm({ onSuccess, onError }) {
  const [amount, setAmount] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);
  const [status, setStatus] = useState(null);
  const [errors, setErrors] = useState({});
  const [reference, setReference] = useState('');

  // Wise recipient state
  const [recipient, setRecipient] = useState(null);
  const [recipientLoading, setRecipientLoading] = useState(!USE_MOCKS);
  const [showAddRecipient, setShowAddRecipient] = useState(false);
  const [recipientForm, setRecipientForm] = useState({
    account_holder_name: '',
    currency: 'USD',
    type: 'email',
    details: { email: '' },
  });
  const [savingRecipient, setSavingRecipient] = useState(false);
  const [recipientError, setRecipientError] = useState(null);

  // Fetch the saved Wise recipient on mount; setting the loading flag at the
  // start of the request is the standard data-fetching effect pattern.
  useEffect(() => {
    if (USE_MOCKS) return;
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setRecipientLoading(true);
    api.developer.getWiseRecipient()
      .then(res => {
        if (!cancelled) setRecipient(res?.data ?? res ?? null);
      })
      .catch(() => { /* 404 = not set yet */ })
      .finally(() => { if (!cancelled) setRecipientLoading(false); });
    return () => { cancelled = true; };
  }, []);

  const handleRecipientTypeChange = (type) => {
    const defaultDetails = type === 'email' ? { email: '' }
      : type === 'iban' ? { iban: '', bic: '' }
      : type === 'ach' ? { account_number: '', routing_number: '' }
      : {};
    setRecipientForm(f => ({ ...f, type, details: defaultDetails }));
    setRecipientError(null);
  };

  const handleSaveRecipient = async (e) => {
    e.preventDefault();
    setSavingRecipient(true);
    setRecipientError(null);
    try {
      const saved = await api.developer.saveWiseRecipient(recipientForm);
      setRecipient(saved?.data ?? saved ?? recipientForm);
      setShowAddRecipient(false);
    } catch (err) {
      setRecipientError(err.message || 'Failed to save recipient.');
    } finally {
      setSavingRecipient(false);
    }
  };

  const validateForm = () => {
    const newErrors = {};
    if (!amount || parseFloat(amount) <= 0) {
      newErrors.amount = 'Please enter a valid amount';
    }
    if (!recipient) {
      newErrors.recipient = 'Please add a Wise payout recipient first';
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
      if (USE_MOCKS) {
        await new Promise(resolve => setTimeout(resolve, 800));
        const txRef = `WD-${Date.now()}`;
        setReference(txRef);
        setStatus('success');
        onSuccess?.({ reference: txRef, amount: parseFloat(amount), method: 'wise' });
        setAmount('');
        setTimeout(() => { setStatus(null); setReference(''); }, 5000);
      } else {
        const result = await api.developer.requestPayout({ amount: parseFloat(amount) });
        const txRef = result?.id ?? `WD-${Date.now()}`;
        setReference(txRef);
        setStatus('success');
        onSuccess?.({ reference: txRef, amount: parseFloat(amount), method: 'wise' });
        setAmount('');
        setTimeout(() => { setStatus(null); setReference(''); }, 5000);
      }
    } catch (err) {
      setStatus('error');
      setErrors({ submit: err.message || 'Payout request failed. Please try again.' });
      onError?.(err);
      setTimeout(() => setStatus(null), 5000);
    }

    setIsProcessing(false);
  };

  const recipientLabel = (r) => {
    if (!r) return '';
    const name = r.account_holder_name ?? r.name ?? '';
    const type = r.type ?? '';
    const detail = r.details?.email ?? r.details?.iban ?? r.details?.account_number ?? '';
    return [name, type, detail].filter(Boolean).join(' · ');
  };

  return (
    <form onSubmit={handleSubmit} className="withdraw-form">
      {/* Wise recipient section */}
      <div style={{ marginBottom: '1rem' }}>
        <label className="input-label">Payout Recipient (Wise)</label>
        {recipientLoading ? (
          <p style={{ color: 'var(--color-text-muted)', fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)' }}>Loading…</p>
        ) : recipient && !showAddRecipient ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', padding: '0.625rem 0.875rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)', marginTop: '0.375rem' }}>
            <span style={{ fontSize: 'var(--text-sm)', fontFamily: 'var(--font-mono)', color: 'var(--color-text-primary)', flex: 1 }}>
              {recipientLabel(recipient)}
            </span>
            <button type="button" className="btn btn-secondary" style={{ fontSize: 'var(--text-xs)', padding: '0.25rem 0.625rem' }} onClick={() => setShowAddRecipient(true)}>
              Change
            </button>
          </div>
        ) : !showAddRecipient ? (
          <button type="button" className="btn btn-secondary" style={{ marginTop: '0.375rem', fontSize: 'var(--text-sm)' }} onClick={() => setShowAddRecipient(true)}>
            + Add Wise Recipient
          </button>
        ) : null}

        {errors.recipient && (
          <p style={{ color: 'var(--color-error)', fontSize: 'var(--text-xs)', marginTop: '0.25rem' }}>{errors.recipient}</p>
        )}

        {showAddRecipient && (
          <div style={{ marginTop: '0.75rem', padding: '0.875rem', background: 'var(--color-bg-elevated)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius)', display: 'flex', flexDirection: 'column', gap: '0.625rem' }}>
            <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
              {['email', 'iban', 'ach'].map(type => (
                <label key={type} style={{ display: 'flex', alignItems: 'center', gap: '0.25rem', cursor: 'pointer', fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)' }}>
                  <input
                    type="radio"
                    name="wfRecipientType"
                    value={type}
                    checked={recipientForm.type === type}
                    onChange={() => handleRecipientTypeChange(type)}
                  />
                  {type.toUpperCase()}
                </label>
              ))}
            </div>
            <input className="input" type="text" placeholder="Account holder name" value={recipientForm.account_holder_name} onChange={e => setRecipientForm(f => ({ ...f, account_holder_name: e.target.value }))} required aria-label="Account holder name" />
            <input className="input" type="text" placeholder="Currency (USD, EUR…)" value={recipientForm.currency} onChange={e => setRecipientForm(f => ({ ...f, currency: e.target.value.toUpperCase() }))} required aria-label="Payout currency" />
            {recipientForm.type === 'email' && (
              <input className="input" type="email" placeholder="Wise email address" value={recipientForm.details.email ?? ''} onChange={e => setRecipientForm(f => ({ ...f, details: { ...f.details, email: e.target.value } }))} required aria-label="Wise email address" />
            )}
            {recipientForm.type === 'iban' && (
              <>
                <input className="input" type="text" placeholder="IBAN" value={recipientForm.details.iban ?? ''} onChange={e => setRecipientForm(f => ({ ...f, details: { ...f.details, iban: e.target.value } }))} required aria-label="IBAN" />
                <input className="input" type="text" placeholder="BIC / SWIFT" value={recipientForm.details.bic ?? ''} onChange={e => setRecipientForm(f => ({ ...f, details: { ...f.details, bic: e.target.value } }))} aria-label="BIC" />
              </>
            )}
            {recipientForm.type === 'ach' && (
              <>
                <input className="input" type="text" placeholder="Account number" value={recipientForm.details.account_number ?? ''} onChange={e => setRecipientForm(f => ({ ...f, details: { ...f.details, account_number: e.target.value } }))} required aria-label="Account number" />
                <input className="input" type="text" placeholder="Routing number" value={recipientForm.details.routing_number ?? ''} onChange={e => setRecipientForm(f => ({ ...f, details: { ...f.details, routing_number: e.target.value } }))} required aria-label="Routing number" />
              </>
            )}
            {recipientError && (
              <p style={{ color: 'var(--color-error)', fontSize: 'var(--text-xs)' }}>{recipientError}</p>
            )}
            <div style={{ display: 'flex', gap: '0.5rem' }}>
              <Button type="button" variant="primary" size="sm" loading={savingRecipient} onClick={handleSaveRecipient}>
                Save
              </Button>
              <Button type="button" variant="secondary" size="sm" onClick={() => { setShowAddRecipient(false); setRecipientError(null); }}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>

      <Input
        label="Amount (USD)"
        type="number"
        placeholder="0.00"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
        error={errors.amount}
        min="0"
        step="0.01"
      />

      {errors.submit && (
        <div className="form-status error">{errors.submit}</div>
      )}

      {status === 'success' && (
        <div className="form-status success">
          Payout requested! Reference: {reference}. Processing via Wise typically takes 1–2 business days.
        </div>
      )}

      <Button
        type="submit"
        variant="primary"
        size="lg"
        loading={isProcessing}
        disabled={!amount || !recipient || parseFloat(amount) <= 0}
        className="withdraw-submit-btn"
      >
        {isProcessing ? 'Processing...' : 'Request Payout via Wise'}
      </Button>

      <p className="withdraw-disclaimer">
        Payouts are processed via Wise (TransferWise). Platform fee: 30%. You receive 70% of earnings.
        Allow 1–2 business days for processing.
      </p>
    </form>
  );
}

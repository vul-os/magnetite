import { useState } from 'react';
import Layout from '../components/Layout';
import { useWallet } from '../hooks/useWallet';
import { useTranslation } from '../i18n/useTranslation';
import { formatReceiptAmount, formatProtocolFee, shortKey } from '../utils/currency';
import './Wallet.css';

/**
 * Wallet — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
 *
 * There is no balance on this page because this node holds no funds. What a
 * wallet *is* here: an address you control, plus the signed receipts that prove
 * what you paid for. Entitlements are read off those receipts — the node grants
 * access by verifying a signature, not by consulting a ledger it owns.
 */

const RECEIPT_KIND_LABEL = {
  item_purchase: 'Item purchase',
  hosting_fee: 'Hosting fee',
  tier: 'Tier',
  wager: 'Wager',
};

export default function Wallet() {
  const { t } = useTranslation();
  const { address, custodial, rail, receipts, link, loading, error } = useWallet();

  const [draftAddress, setDraftAddress] = useState('');
  const [linking, setLinking] = useState(false);
  const [linkError, setLinkError] = useState(null);
  const [copied, setCopied] = useState(false);

  const linkInputId = 'wallet-link-address';

  const formatDate = (value) => {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return '—';
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  };

  const formatTime = (value) => {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
  };

  const handleLink = async (e) => {
    e.preventDefault();
    if (!draftAddress.trim()) return;
    setLinkError(null);
    setLinking(true);
    try {
      await link(draftAddress);
      setDraftAddress('');
    } catch (err) {
      setLinkError(err.message || 'Could not link that wallet address.');
    } finally {
      setLinking(false);
    }
  };

  const handleCopy = async () => {
    if (!address) return;
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      /* clipboard unavailable — the address is selectable in the DOM anyway */
    }
  };

  const settled = receipts.filter((r) => !r.voided);
  const totalSettled = settled.reduce((sum, r) => sum + (Number(r.total) || 0), 0);

  return (
    <Layout>
      <div className="wallet">
        <header className="wallet-page-header">
          <span className="kicker">// {t('walletPage.kicker')}</span>
          <h1>{t('walletPage.title')}</h1>
          <p className="wallet-subtitle">{t('walletPage.subtitle')}</p>
        </header>

        {error && (
          <div className="wallet-alert" role="alert">
            {error}
          </div>
        )}

        <div className="wallet-grid">
          <div className="wallet-left">
            {/* ── Custody posture ─────────────────────────────────────────── */}
            <section className="custody-card" aria-label={t('walletPage.custodyLabel')}>
              <div className="custody-top">
                <span className="custody-badge" data-custodial={String(custodial)}>
                  <span className="custody-dot" aria-hidden="true" />
                  {t('walletPage.nonCustodial')}
                </span>
                <span className="custody-rail">
                  {t('walletPage.rail')}: <code>{rail || '—'}</code>
                </span>
              </div>
              <p className="custody-explainer">{t('walletPage.custodyExplainer')}</p>
              <dl className="custody-facts">
                <div>
                  <dt>{t('walletPage.custodyField')}</dt>
                  <dd>
                    <code>custodial: false</code>
                  </dd>
                </div>
                <div>
                  <dt>{t('walletPage.protocolFee')}</dt>
                  <dd>{formatProtocolFee(0)}</dd>
                </div>
              </dl>
            </section>

            {/* ── Linked address ──────────────────────────────────────────── */}
            <section className="address-card" aria-label={t('walletPage.addressLabel')}>
              <span className="kicker">// {t('walletPage.addressKicker')}</span>
              <h3>{t('walletPage.linkedWallet')}</h3>

              {loading ? (
                <p className="wallet-muted">{t('common.loading')}</p>
              ) : address ? (
                <>
                  <div className="address-row">
                    <code className="address-value" title={address}>
                      {address}
                    </code>
                    <button
                      type="button"
                      className="btn btn-secondary btn-copy"
                      onClick={handleCopy}
                      aria-label={t('walletPage.copyAddress')}
                    >
                      {copied ? t('walletPage.copied') : t('walletPage.copy')}
                    </button>
                  </div>
                  <p className="wallet-muted address-note">{t('walletPage.addressNote')}</p>
                </>
              ) : (
                <p className="wallet-muted address-note">{t('walletPage.noWallet')}</p>
              )}

              <form className="link-form" onSubmit={handleLink}>
                <label htmlFor={linkInputId} className="link-label">
                  {address ? t('walletPage.replaceWallet') : t('walletPage.linkWallet')}
                </label>
                <div className="link-row">
                  <input
                    id={linkInputId}
                    className="link-input"
                    type="text"
                    inputMode="latin"
                    spellCheck="false"
                    autoComplete="off"
                    placeholder={t('walletPage.addressPlaceholder')}
                    value={draftAddress}
                    onChange={(e) => setDraftAddress(e.target.value)}
                  />
                  <button
                    type="submit"
                    className="btn btn-primary"
                    disabled={!draftAddress.trim() || linking}
                  >
                    {linking ? t('walletPage.linking') : t('walletPage.link')}
                  </button>
                </div>
                {linkError && (
                  <p className="wallet-error" role="alert">
                    {linkError}
                  </p>
                )}
                <p className="wallet-muted link-hint">{t('walletPage.linkHint')}</p>
              </form>
            </section>
          </div>

          <div className="wallet-right">
            <section className="receipts-card" aria-label={t('walletPage.receiptsLabel')}>
              <div className="receipts-header">
                <div>
                  <h3>{t('walletPage.receipts')}</h3>
                  <p className="wallet-muted receipts-sub">{t('walletPage.receiptsSub')}</p>
                </div>
                <div className="receipts-total">
                  <span className="receipts-total-label">{t('walletPage.totalSettled')}</span>
                  <span className="receipts-total-value">
                    {formatReceiptAmount(totalSettled)}
                  </span>
                </div>
              </div>

              {loading ? (
                <div className="loading-state">
                  <span className="spinner" aria-hidden="true" />
                  <span>{t('walletPage.loadingReceipts')}</span>
                </div>
              ) : receipts.length === 0 ? (
                <div className="receipts-empty">{t('walletPage.noReceipts')}</div>
              ) : (
                <ul className="receipts-list">
                  {receipts.map((r) => (
                    <li
                      key={r.id}
                      className={`receipt-item${r.voided ? ' receipt-voided' : ''}`}
                    >
                      <div className="receipt-main">
                        <div className="receipt-headline">
                          <span className="receipt-subject">
                            {r.subject || RECEIPT_KIND_LABEL[r.kind] || r.kind}
                          </span>
                          <span className="receipt-kind">
                            {RECEIPT_KIND_LABEL[r.kind] || r.kind}
                          </span>
                        </div>
                        <div className="receipt-meta">
                          {formatDate(r.created_at)} · {formatTime(r.created_at)}
                          {r.counterparty && (
                            <>
                              {' '}
                              · {t('walletPage.paidTo')}{' '}
                              <code title={r.counterparty}>{shortKey(r.counterparty)}</code>
                            </>
                          )}
                        </div>
                        <div className="receipt-proof">
                          <span className="receipt-proof-item">
                            <span className="receipt-proof-key">{t('walletPage.receiptId')}</span>
                            <code>{r.id}</code>
                          </span>
                          <span className="receipt-proof-item">
                            <span className="receipt-proof-key">{t('walletPage.signedBy')}</span>
                            <code title={r.rail_pubkey}>{shortKey(r.rail_pubkey)}</code>
                          </span>
                          <span className="receipt-proof-item">
                            <span className="receipt-proof-key">{t('walletPage.fee')}</span>
                            <code>{formatReceiptAmount(r.protocol_fee)}</code>
                          </span>
                        </div>
                      </div>
                      <div className="receipt-right">
                        <span className="receipt-amount">{formatReceiptAmount(r.total)}</span>
                        <span
                          className={`receipt-status ${r.voided ? 'status-voided' : 'status-settled'}`}
                        >
                          {r.voided ? t('walletPage.voided') : t('walletPage.settled')}
                        </span>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </div>
        </div>
      </div>
    </Layout>
  );
}

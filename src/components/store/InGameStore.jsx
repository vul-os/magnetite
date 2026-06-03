/**
 * InGameStore — in-game purchase panel.
 *
 * Props:
 *   storeId       {string}   — store to display (loads items lazily)
 *   gameTitle     {string}   — game name shown in the header
 *   onClose       {function} — called when the panel is dismissed
 *   pointBalance  {number}   — current player points (from parent context)
 *   usdcBalance   {number}   — current USD wallet balance (optional)
 *
 * Purchase flow:
 *   1. Click "Buy" → confirm modal shown
 *   2. Confirm → API called → success receipt / error feedback
 *   3. Owned items show "Owned" badge; previously purchased show receipt
 */
import { useState, useEffect, useCallback, useRef } from 'react';
import { useMarketplace } from '../../hooks/useMarketplace';
import './InGameStore.css';

// ── Icons ─────────────────────────────────────────────────────────────────────

function CloseIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6"  y1="6" x2="18" y2="18" />
    </svg>
  );
}

function CoinsIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <circle cx="8"  cy="8"  r="6" />
      <path d="M18.09 10.37A6 6 0 1 1 10.34 18" />
      <path d="M7 6h1v4" />
      <line x1="16.71" y1="13.88" x2="13.12" y2="17.47" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

function ShieldIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
    </svg>
  );
}

function AlertIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const ITEM_TYPE_LABELS = {
  cosmetic: 'Cosmetic',
  boost:    'Boost',
  bundle:   'Bundle',
  currency: 'Currency',
  other:    'Item',
};

const MOCK_FALLBACK_ITEMS = [
  { id: 'igs1', name: 'Neon Helm Skin',    description: 'Glowing teal helmet.',      price_points: 800,  price_usdc: 0.99, item_type: 'cosmetic', active: true },
  { id: 'igs2', name: 'XP Accelerator',   description: '2× XP for 24 hours.',       price_points: 500,  price_usdc: 0.49, item_type: 'boost',    active: true },
  { id: 'igs3', name: 'Starter Bundle',   description: 'Skin + boost combo deal.',   price_points: 1200, price_usdc: 1.49, item_type: 'bundle',   active: true },
  { id: 'igs4', name: 'Point Pack (500)', description: 'Top-up 500 bonus points.',   price_points: 0,    price_usdc: 0.99, item_type: 'currency', active: true },
  { id: 'igs5', name: 'Carbon Visor',     description: 'Matte-black visor skin.',    price_points: 600,  price_usdc: 0.79, item_type: 'cosmetic', active: true },
  { id: 'igs6', name: 'Drop Shield Pack', description: 'Three extra shield drops.',  price_points: 400,  price_usdc: 0.39, item_type: 'boost',    active: true },
];

// ── Confirm Modal ─────────────────────────────────────────────────────────────

function ConfirmPurchaseModal({ item, priceLabel, onConfirm, onCancel, confirming }) {
  const cancelRef = useRef(null);

  // Focus the cancel button on open for safer UX
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  function handleKeyDown(e) {
    if (e.key === 'Escape') onCancel();
  }

  return (
    <div
      className="igs-confirm-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="igs-confirm-title"
      onKeyDown={handleKeyDown}
    >
      <div className="igs-confirm-sheet">
        <div className="igs-confirm-icon-wrap" aria-hidden="true">
          <ShieldIcon />
        </div>

        <h3 id="igs-confirm-title" className="igs-confirm-title">Confirm Purchase</h3>

        <div className="igs-confirm-item-preview">
          <img
            src={`https://picsum.photos/seed/${item.id}/80/56`}
            alt={item.name}
            className="igs-confirm-item-img"
            width={80}
            height={56}
          />
          <div className="igs-confirm-item-info">
            <span className="igs-confirm-item-name">{item.name}</span>
            <span className="igs-confirm-item-type">{ITEM_TYPE_LABELS[item.item_type] ?? item.item_type}</span>
            <span className="igs-confirm-item-desc">{item.description}</span>
          </div>
        </div>

        <div className="igs-confirm-price-row">
          <span className="igs-confirm-price-label">You pay</span>
          <span className="igs-confirm-price-value">{priceLabel === '0 pts' || priceLabel === '$0.00' ? 'Free' : priceLabel}</span>
        </div>

        <p className="igs-confirm-notice">
          Purchases are tied to your Magnetite account and are final.
        </p>

        <div className="igs-confirm-actions">
          <button
            ref={cancelRef}
            className="btn btn-secondary igs-confirm-cancel"
            onClick={onCancel}
            disabled={confirming}
          >
            Cancel
          </button>
          <button
            className="btn btn-primary igs-confirm-ok"
            onClick={onConfirm}
            disabled={confirming}
            aria-busy={confirming}
          >
            {confirming ? (
              <span className="igs-btn-spinner" aria-hidden="true" />
            ) : (
              'Buy Now'
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Receipt / Success overlay ──────────────────────────────────────────────────

function ReceiptToast({ item, priceLabel, onDismiss }) {
  useEffect(() => {
    const t = setTimeout(onDismiss, 3500);
    return () => clearTimeout(t);
  }, [onDismiss]);

  return (
    <div className="igs-receipt" role="status" aria-live="polite">
      <span className="igs-receipt-icon" aria-hidden="true">
        <CheckIcon />
      </span>
      <span className="igs-receipt-text">
        <strong>{item.name}</strong> purchased for {priceLabel}!
      </span>
      <button className="igs-receipt-dismiss" onClick={onDismiss} aria-label="Dismiss receipt">
        <CloseIcon />
      </button>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────

export function InGameStore({
  storeId,
  gameTitle = 'Game Store',
  onClose,
  pointBalance = 2500,
  usdcBalance = 10.0,
}) {
  const { items, loadItems, purchase, hasEntitlement, purchasing } = useMarketplace();

  const [currency, setCurrency]     = useState('points');  // 'points' | 'usdc'
  const [filterType, setFilterType] = useState('all');

  // Confirm modal state
  const [confirmItem, setConfirmItem] = useState(null);  // item to confirm
  const [confirming,  setConfirming]  = useState(false); // mid-request

  // Per-item status: 'success' | 'error' | 'insufficient' | null
  const [statusMap, setStatusMap]   = useState({});

  // Receipt toast
  const [receipt, setReceipt] = useState(null); // { item, priceLabel }

  // Load items on mount
  useEffect(() => {
    if (storeId) {
      loadItems(storeId);
    }
  }, [storeId, loadItems]);

  const storeItems = storeId
    ? (items[storeId] ?? MOCK_FALLBACK_ITEMS)
    : MOCK_FALLBACK_ITEMS;

  const activeItems = storeItems.filter(i => i.active !== false);
  const types       = ['all', ...new Set(activeItems.map(i => i.item_type))];
  const displayed   = filterType === 'all' ? activeItems : activeItems.filter(i => i.item_type === filterType);

  // ── Purchase helpers ────────────────────────────────────────────────────────

  function getPriceLabel(item) {
    return currency === 'points'
      ? `${item.price_points.toLocaleString()} pts`
      : `$${Number(item.price_usdc).toFixed(2)}`;
  }

  function canAffordItem(item) {
    return currency === 'points'
      ? pointBalance >= item.price_points
      : usdcBalance  >= item.price_usdc;
  }

  function isItemFree(item) {
    return currency === 'points' ? item.price_points === 0 : item.price_usdc === 0;
  }

  // Clicking "Buy" on an item — show confirm modal (or proceed for free items)
  const handleBuyClick = useCallback((item) => {
    if (!canAffordItem(item)) {
      setStatusMap(m => ({ ...m, [item.id]: 'insufficient' }));
      setTimeout(() => setStatusMap(m => ({ ...m, [item.id]: null })), 2500);
      return;
    }
    setConfirmItem(item);
  }, [currency, pointBalance, usdcBalance]); // eslint-disable-line react-hooks/exhaustive-deps

  // Confirmed — call API
  const handleConfirmPurchase = useCallback(async () => {
    if (!confirmItem) return;
    const item = confirmItem;
    setConfirming(true);

    const result = await purchase(storeId ?? 'default', item.id, currency);
    setConfirming(false);
    setConfirmItem(null);

    if (result.success) {
      const priceLabel = isItemFree(item) ? 'Free' : getPriceLabel(item);
      setReceipt({ item, priceLabel });
      setStatusMap(m => ({ ...m, [item.id]: 'success' }));
    } else {
      setStatusMap(m => ({ ...m, [item.id]: 'error' }));
      setTimeout(() => setStatusMap(m => ({ ...m, [item.id]: null })), 3000);
    }
  }, [confirmItem, storeId, currency, purchase]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleCancelConfirm = useCallback(() => {
    setConfirmItem(null);
    setConfirming(false);
  }, []);

  // Trap Escape key inside panel
  function handleKeyDown(e) {
    if (e.key === 'Escape') {
      if (confirmItem) {
        handleCancelConfirm();
      } else {
        onClose?.();
      }
    }
  }

  const isLoading = storeId && items[storeId] === undefined;

  return (
    <div
      className="igs-panel"
      role="dialog"
      aria-modal="true"
      aria-label={`${gameTitle} Store`}
      onKeyDown={handleKeyDown}
    >
      {/* ── Header ─────────────────────────────────────────────────────────── */}
      <header className="igs-header">
        <div className="igs-header-left">
          <span className="igs-kicker">// In-Game Store</span>
          <h2 className="igs-title">{gameTitle}</h2>
        </div>
        <div className="igs-header-right">
          <div className="igs-balance-row">
            <span className="igs-balance-item">
              <span className="igs-coin-icon" aria-hidden="true">⬡</span>
              <span className="igs-balance-val">{pointBalance.toLocaleString()}</span>
              <span className="igs-balance-label">pts</span>
            </span>
            <span className="igs-balance-sep" aria-hidden="true">·</span>
            <span className="igs-balance-item">
              <span className="igs-usdc-icon" aria-hidden="true">◎</span>
              <span className="igs-balance-val">${usdcBalance.toFixed(2)}</span>
            </span>
          </div>
          {onClose && (
            <button className="igs-close-btn" onClick={onClose} aria-label="Close store">
              <CloseIcon />
            </button>
          )}
        </div>
      </header>

      {/* ── Currency toggle + type filter ──────────────────────────────────── */}
      <div className="igs-controls">
        <div className="igs-currency-toggle" role="group" aria-label="Payment currency">
          <button
            className={`igs-curr-btn${currency === 'points' ? ' active' : ''}`}
            onClick={() => setCurrency('points')}
            aria-pressed={currency === 'points'}
          >
            <span aria-hidden="true">⬡</span> Points
          </button>
          <button
            className={`igs-curr-btn${currency === 'usdc' ? ' active' : ''}`}
            onClick={() => setCurrency('usdc')}
            aria-pressed={currency === 'usdc'}
          >
            <span aria-hidden="true">◎</span> USDC
          </button>
        </div>

        {types.length > 1 && (
          <div className="igs-type-filters" role="group" aria-label="Item category">
            {types.map(t => (
              <button
                key={t}
                className={`igs-filter-btn${filterType === t ? ' active' : ''}`}
                onClick={() => setFilterType(t)}
                aria-pressed={filterType === t}
              >
                {t === 'all' ? 'All' : ITEM_TYPE_LABELS[t] ?? t}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* ── Receipt toast ──────────────────────────────────────────────────── */}
      {receipt && (
        <ReceiptToast
          item={receipt.item}
          priceLabel={receipt.priceLabel}
          onDismiss={() => setReceipt(null)}
        />
      )}

      {/* ── Items grid ─────────────────────────────────────────────────────── */}
      <div className="igs-body">
        {isLoading ? (
          <div className="igs-loading" role="status" aria-live="polite">
            <span className="igs-spinner" aria-hidden="true" />
            <span>Loading items…</span>
          </div>
        ) : displayed.length === 0 ? (
          <div className="igs-empty" role="status">
            <CoinsIcon />
            <p>No items available in this category.</p>
          </div>
        ) : (
          <ul className="igs-item-grid" role="list">
            {displayed.map(item => {
              const owned      = hasEntitlement(item.id);
              const status     = statusMap[item.id];
              const priceLabel = getPriceLabel(item);
              const canAfford  = canAffordItem(item);
              const free       = isItemFree(item);
              const isSuccess  = status === 'success';

              return (
                <li key={item.id} className="igs-item-card" role="listitem">
                  <div className="igs-item-img-wrap" aria-hidden="true">
                    <img
                      src={`https://picsum.photos/seed/${item.id}/120/80`}
                      alt=""
                      className="igs-item-img"
                      width={120}
                      height={80}
                      loading="lazy"
                    />
                    <span className="igs-item-type-badge">
                      {ITEM_TYPE_LABELS[item.item_type] ?? item.item_type}
                    </span>
                  </div>

                  <div className="igs-item-body">
                    <h3 className="igs-item-name">{item.name}</h3>
                    <p  className="igs-item-desc">{item.description}</p>
                  </div>

                  <div className="igs-item-footer">
                    <span className={`igs-item-price${!canAfford && !owned ? ' cant-afford' : ''}`}>
                      {free ? 'Free' : priceLabel}
                    </span>

                    {owned || isSuccess ? (
                      <span className="igs-owned-badge" aria-label={owned ? 'Already owned' : 'Item purchased'}>
                        <CheckIcon /> {owned ? 'Owned' : 'Purchased!'}
                      </span>
                    ) : status === 'insufficient' ? (
                      <span className="igs-insufficient-badge" role="alert">
                        <AlertIcon /> Not enough {currency === 'points' ? 'pts' : 'USDC'}
                      </span>
                    ) : status === 'error' ? (
                      <span className="igs-error-badge" role="alert">
                        <AlertIcon /> Failed — try again
                      </span>
                    ) : (
                      <button
                        className={`btn btn-sm ${canAfford ? 'btn-primary' : 'btn-secondary'} igs-buy-btn`}
                        onClick={() => handleBuyClick(item)}
                        disabled={purchasing || !canAfford}
                        aria-label={
                          !canAfford
                            ? `Not enough ${currency === 'points' ? 'pts' : 'USDC'} to buy ${item.name}`
                            : `Buy ${item.name} for ${free ? 'free' : priceLabel}`
                        }
                      >
                        {purchasing ? '…' : canAfford ? 'Buy' : 'Not Enough'}
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* ── Footer ─────────────────────────────────────────────────────────── */}
      <footer className="igs-footer">
        <span className="igs-footer-note">
          Items are tied to your Magnetite account. Purchases are final.
        </span>
      </footer>

      {/* ── Confirm modal ──────────────────────────────────────────────────── */}
      {confirmItem && (
        <ConfirmPurchaseModal
          item={confirmItem}
          priceLabel={isItemFree(confirmItem) ? 'Free' : getPriceLabel(confirmItem)}
          onConfirm={handleConfirmPurchase}
          onCancel={handleCancelConfirm}
          confirming={confirming}
        />
      )}
    </div>
  );
}

export default InGameStore;

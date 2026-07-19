/**
 * Money formatting — NON-CUSTODIAL (seam §3.6 `PaymentRail`).
 *
 * There is no fiat here. No ZAR, no Paystack, no Wise, no custodial balance.
 * Value moves buyer-wallet → developer-wallet on a crypto rail (USDC by
 * default) and the only artefact the UI ever sees is a *signed receipt*.
 *
 * Receipts carry amounts as integers in the rail's **smallest unit** (USDC has
 * 6 decimals on-chain, but this node's rail quotes cents — see
 * `RAIL_DECIMALS`). Always render through `formatUSDC` so the whole app agrees
 * on how many decimals a receipt has.
 */

/** Decimal places used by receipt `total` / `protocol_fee` integers. */
export const RAIL_DECIMALS = 2;

const SMALLEST_UNITS_PER_TOKEN = 10 ** RAIL_DECIMALS;

/**
 * Format a decimal token amount (e.g. `4.99`) with its ticker.
 * Use `formatReceiptAmount` when you have raw smallest-unit integers.
 */
export function formatCurrency(amount, currency = 'USDC') {
  const formatted = new Intl.NumberFormat('en-US', {
    style: 'decimal',
    minimumFractionDigits: RAIL_DECIMALS,
    maximumFractionDigits: RAIL_DECIMALS,
  }).format(Number(amount) || 0);

  return `${formatted} ${currency}`;
}

/** Format a decimal token amount as USDC — the default rail currency. */
export function formatUSDC(amount) {
  return formatCurrency(amount, 'USDC');
}

/**
 * Format a raw receipt integer (smallest unit) as a human token amount.
 * `formatReceiptAmount(499)` → `"4.99 USDC"`.
 */
export function formatReceiptAmount(smallestUnits, currency = 'USDC') {
  const tokens = (Number(smallestUnits) || 0) / SMALLEST_UNITS_PER_TOKEN;
  return formatCurrency(tokens, currency);
}

/** Convert a decimal token amount to the integer a checkout call expects. */
export function toSmallestUnits(amount) {
  return Math.round((Number(amount) || 0) * SMALLEST_UNITS_PER_TOKEN);
}

/** Render a protocol fee in basis points. `0` → `"0 bps (no protocol fee)"`. */
export function formatProtocolFee(bps) {
  const n = Number(bps) || 0;
  return n === 0 ? '0 bps (no protocol fee)' : `${n} bps`;
}

/**
 * Abbreviate a hex Ed25519 key for display. Raw keys are the substrate; any
 * human name is a display layer on top (seam §3.2 `Naming`).
 */
export function shortKey(hexKey, lead = 6, tail = 4) {
  if (!hexKey) return '—';
  const clean = String(hexKey).replace(/^0x/, '');
  if (clean.length <= lead + tail + 1) return clean;
  return `${clean.slice(0, lead)}…${clean.slice(-tail)}`;
}

export function parseCurrency(value) {
  const cleaned = String(value).replace(/[^0-9.-]/g, '');
  return parseFloat(cleaned) || 0;
}

export function formatCompactNumber(num) {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M';
  }
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K';
  }
  return num.toString();
}

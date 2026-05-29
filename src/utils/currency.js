export function formatCurrency(amount, currency = 'USDC') {
  const formatted = new Intl.NumberFormat('en-US', {
    style: 'decimal',
    minimumFractionDigits: 2,
    maximumFractionDigits: 6,
  }).format(amount);

  return `${formatted} ${currency}`;
}

export function formatUSD(amount) {
  return formatCurrency(amount, 'USD');
}

export function formatUSDC(amount) {
  return formatCurrency(amount, 'USDC');
}

export function parseCurrency(value) {
  const cleaned = value.replace(/[^0-9.-]/g, '');
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

import { useState } from 'react';
import { AreaChart as RechartsAreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

const colors = {
  primary:    '#f5a524',   /* --color-amber */
  secondary:  '#3ddc84',   /* --color-success */
  tertiary:   '#5b9dff',   /* --color-info */
  grid:       '#23232e',   /* --color-border */
  text:       '#6b6b78',   /* --color-text-muted */
  background: '#111319',   /* --color-bg-card */
  border:     '#23232e',   /* --color-border */
};

function CustomTooltip({ active, payload, label }) {
  if (!active || !payload || !payload.length) return null;

  return (
    <div style={{
      background: colors.background,
      border: `1px solid ${colors.grid}`,
      borderRadius: 8,
      padding: '12px 16px',
    }}>
      <p style={{ color: '#e4e4e7', marginBottom: 8, fontWeight: 600 }}>{label}</p>
      {payload.map((entry, index) => (
        <p key={index} style={{ color: entry.color, margin: '4px 0', fontSize: 13 }}>
          {entry.name}: {typeof entry.value === 'number' ? `$${entry.value.toLocaleString()}` : entry.value}
        </p>
      ))}
    </div>
  );
}

function formatTimeAxis(value, period) {
  if (!value) return '';
  const date = new Date(value);
  if (isNaN(date.getTime())) return value;
  if (period === 'daily') {
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  }
  if (period === 'weekly') {
    return `Week ${date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
  }
  return date.toLocaleDateString('en-US', { month: 'short', year: '2-digit' });
}

function calculateChange(current, previous) {
  if (!previous || previous === 0) return null;
  return ((current - previous) / previous * 100).toFixed(1);
}

export default function RevenueChart({
  data = [],
  title = 'Revenue Overview',
  height = 350,
}) {
  const [period, setPeriod] = useState('daily');

  const aggregatedData = data.reduce((acc, item) => {
    const date = new Date(item.date);
    let key;
    if (period === 'daily') {
      key = item.date;
    } else if (period === 'weekly') {
      const weekStart = new Date(date);
      weekStart.setDate(date.getDate() - date.getDay());
      key = weekStart.toISOString().split('T')[0];
    } else {
      key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`;
    }

    const existing = acc.find(a => a.date === key);
    if (existing) {
      existing.platformFees += item.platformFees || 0;
      existing.developerEarnings += item.developerEarnings || 0;
      existing.totalRevenue += item.totalRevenue || 0;
    } else {
      acc.push({
        date: key,
        platformFees: item.platformFees || 0,
        developerEarnings: item.developerEarnings || 0,
        totalRevenue: item.totalRevenue || (item.platformFees || 0) + (item.developerEarnings || 0),
      });
    }
    return acc;
  }, []);

  aggregatedData.sort((a, b) => new Date(a.date) - new Date(b.date));

  const currentPeriodTotals = aggregatedData.slice(-7).reduce((sum, d) => ({
    revenue: sum.revenue + d.totalRevenue,
    earnings: sum.earnings + d.developerEarnings,
  }), { revenue: 0, earnings: 0 });

  const prevPeriodTotals = aggregatedData.slice(-14, -7).reduce((sum, d) => ({
    revenue: sum.revenue + d.totalRevenue,
    earnings: sum.earnings + d.developerEarnings,
  }), { revenue: 0, earnings: 0 });

  const revenueChange = calculateChange(currentPeriodTotals.revenue, prevPeriodTotals.revenue);
  const earningsChange = calculateChange(currentPeriodTotals.earnings, prevPeriodTotals.earnings);

  return (
    <div className="chart-container">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        {title && <h4 className="chart-title" style={{ margin: 0 }}>{title}</h4>}
        <div style={{ display: 'flex', gap: 8 }}>
          {['daily', 'weekly', 'monthly'].map(p => (
            <button
              key={p}
              onClick={() => setPeriod(p)}
              style={{
                padding: '6px 12px',
                borderRadius: 6,
                border: '1px solid',
                borderColor: period === p ? colors.primary : colors.border,
                background: period === p ? `${colors.primary}20` : 'transparent',
                color: period === p ? colors.primary : colors.text,
                fontSize: 12,
                cursor: 'pointer',
                textTransform: 'capitalize',
              }}
            >
              {p}
            </button>
          ))}
        </div>
      </div>

      <div style={{ display: 'flex', gap: 24, marginBottom: 20 }}>
        <div>
          <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>Total Revenue</p>
          <p style={{ color: colors.primary, fontSize: 20, fontWeight: 600, margin: '4px 0' }}>
            ${currentPeriodTotals.revenue.toLocaleString()}
          </p>
          {revenueChange && (
            <p style={{
              color: parseFloat(revenueChange) >= 0 ? colors.secondary : '#ef4444',
              fontSize: 12,
              margin: 0,
            }}>
              {parseFloat(revenueChange) >= 0 ? '+' : ''}{revenueChange}% vs prev period
            </p>
          )}
        </div>
        <div>
          <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>Developer Earnings</p>
          <p style={{ color: colors.secondary, fontSize: 20, fontWeight: 600, margin: '4px 0' }}>
            ${currentPeriodTotals.earnings.toLocaleString()}
          </p>
          {earningsChange && (
            <p style={{
              color: parseFloat(earningsChange) >= 0 ? colors.secondary : '#ef4444',
              fontSize: 12,
              margin: 0,
            }}>
              {parseFloat(earningsChange) >= 0 ? '+' : ''}{earningsChange}% vs prev period
            </p>
          )}
        </div>
      </div>

      <ResponsiveContainer width="100%" height={height}>
        <RechartsAreaChart data={aggregatedData} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
          <defs>
            <linearGradient id="platformFeeGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={colors.tertiary} stopOpacity={0.4} />
              <stop offset="95%" stopColor={colors.tertiary} stopOpacity={0.05} />
            </linearGradient>
            <linearGradient id="earningsGradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={colors.secondary} stopOpacity={0.4} />
              <stop offset="95%" stopColor={colors.secondary} stopOpacity={0.05} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
          <XAxis
            dataKey="date"
            stroke={colors.text}
            tick={{ fill: colors.text, fontSize: 12 }}
            tickFormatter={(v) => formatTimeAxis(v, period)}
          />
          <YAxis
            stroke={colors.text}
            tick={{ fill: colors.text, fontSize: 12 }}
            tickFormatter={(v) => `$${v >= 1000 ? `${(v/1000).toFixed(0)}k` : v}`}
          />
          <Tooltip content={<CustomTooltip />} cursor={{ stroke: colors.grid, strokeWidth: 1 }} />
          <Legend
            wrapperStyle={{ color: colors.text, fontSize: 12, paddingTop: 10 }}
            iconType="circle"
            iconSize={8}
          />
          <Area
            type="monotone"
            dataKey="platformFees"
            name="Platform Fees"
            stroke={colors.tertiary}
            strokeWidth={2}
            fill="url(#platformFeeGradient)"
          />
          <Area
            type="monotone"
            dataKey="developerEarnings"
            name="Developer Earnings"
            stroke={colors.secondary}
            strokeWidth={2}
            fill="url(#earningsGradient)"
          />
        </RechartsAreaChart>
      </ResponsiveContainer>
    </div>
  );
}

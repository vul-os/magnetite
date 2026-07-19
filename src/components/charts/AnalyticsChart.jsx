/**
 * AnalyticsChart — dual-series time-series chart for the Developer Analytics page.
 * Renders either revenue-over-time (USD) or playtime-over-time (minutes) from a flat
 * [{ date, value }] series.  Styled to the Industrial Magnetite dark palette.
 */
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

/* ── Design tokens (match CSS custom properties) ─────────────────────────── */
const C = {
  amber:   '#f5a524',
  cyan:    '#7b61ff',
  grid:    '#23232e',
  text:    '#6b6b78',
  bg:      '#111319',
  border:  '#23232e',
};

/* ── Custom tooltip ──────────────────────────────────────────────────────── */
function ChartTooltip({ active, payload, label, formatter }) {
  if (!active || !payload || !payload.length) return null;
  const raw = payload[0]?.value ?? 0;
  return (
    <div style={{
      background: C.bg,
      border: `1px solid ${C.border}`,
      borderRadius: 8,
      padding: '10px 14px',
      boxShadow: '0 8px 24px rgba(0,0,0,0.5)',
    }}>
      <p style={{ margin: '0 0 6px', fontFamily: 'JetBrains Mono, monospace', fontSize: 11, color: C.text, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
        {label}
      </p>
      <p style={{ margin: 0, fontFamily: 'JetBrains Mono, monospace', fontSize: 17, fontWeight: 700, color: payload[0]?.color ?? C.amber }}>
        {formatter ? formatter(raw) : raw}
      </p>
    </div>
  );
}

/* ── Short-date label (May 15, Jun 1, …) ─────────────────────────────────── */
function shortDate(str) {
  if (!str) return '';
  const d = new Date(str + 'T00:00:00');
  if (isNaN(d.getTime())) return str;
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

/* ── AnalyticsChart ──────────────────────────────────────────────────────── */
export default function AnalyticsChart({
  data = [],           // [{ date: 'YYYY-MM-DD', value: number }]
  color = 'amber',     // 'amber' | 'cyan'
  gradientId,          // unique id for the SVG gradient — must be unique per page
  yFormatter,          // (value) => string
  tooltipFormatter,    // (value) => string
  height = 260,
  emptyMessage = 'No data for this period.',
}) {
  const stroke = color === 'cyan' ? C.cyan : C.amber;
  const gId = gradientId ?? `grad-${color}`;

  if (!data || data.length === 0) {
    return (
      <div style={{ height, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <p style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: 13, color: C.text }}>
          {emptyMessage}
        </p>
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id={gId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%"  stopColor={stroke} stopOpacity={0.28} />
            <stop offset="95%" stopColor={stroke} stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid strokeDasharray="3 3" stroke={C.grid} vertical={false} />
        <XAxis
          dataKey="date"
          stroke={C.text}
          tick={{ fill: C.text, fontSize: 11, fontFamily: 'JetBrains Mono, monospace' }}
          tickLine={false}
          axisLine={false}
          tickFormatter={shortDate}
          interval="preserveStartEnd"
        />
        <YAxis
          stroke={C.text}
          tick={{ fill: C.text, fontSize: 11, fontFamily: 'JetBrains Mono, monospace' }}
          tickLine={false}
          axisLine={false}
          tickFormatter={yFormatter ?? ((v) => v)}
          width={56}
        />
        <Tooltip
          content={<ChartTooltip formatter={tooltipFormatter} />}
          cursor={{ stroke: C.grid, strokeWidth: 1 }}
        />
        <Area
          type="monotone"
          dataKey="value"
          stroke={stroke}
          strokeWidth={2}
          fill={`url(#${gId})`}
          dot={false}
          activeDot={{ r: 4, fill: stroke, strokeWidth: 0 }}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}

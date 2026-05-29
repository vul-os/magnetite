import { LineChart as RechartsLineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

const colors = {
  primary: '#f59e0b',
  secondary: '#f97316',
  tertiary: '#3b82f6',
  quaternary: '#22c55e',
  grid: '#27272a',
  text: '#a1a1aa',
  background: '#1a1a25',
};

const SERIES_COLORS = [colors.primary, colors.secondary, colors.tertiary, colors.quaternary, '#8b5cf6', '#ec4899'];

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
          {entry.name}: {typeof entry.value === 'number' ? entry.value.toLocaleString() : entry.value}
        </p>
      ))}
    </div>
  );
}

function formatTimeAxis(value) {
  if (!value) return '';
  const date = new Date(value);
  if (isNaN(date.getTime())) return value;
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

export default function LineChart({
  data,
  series = [],
  title,
  showGrid = true,
  showLegend = true,
  showTooltip = true,
  height = 300,
}) {
  const xKey = series.length > 0 ? series[0].xKey || 'date' : 'date';

  return (
    <div className="chart-container">
      {title && <h4 className="chart-title">{title}</h4>}
      <ResponsiveContainer width="100%" height={height}>
        <RechartsLineChart data={data} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
          <defs>
            {series.map((s, i) => (
              <linearGradient key={`gradient-${i}`} id={`lineGradient-${i}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={SERIES_COLORS[i % SERIES_COLORS.length]} stopOpacity={0.3} />
                <stop offset="95%" stopColor={SERIES_COLORS[i % SERIES_COLORS.length]} stopOpacity={0} />
              </linearGradient>
            ))}
          </defs>
          {showGrid && <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />}
          <XAxis
            dataKey={xKey}
            stroke={colors.text}
            tick={{ fill: colors.text, fontSize: 12 }}
            tickFormatter={formatTimeAxis}
          />
          <YAxis stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
          {showTooltip && (
            <Tooltip
              content={<CustomTooltip />}
              cursor={{ stroke: colors.grid, strokeWidth: 1 }}
            />
          )}
          {showLegend && series.length > 1 && (
            <Legend
              wrapperStyle={{ color: colors.text, fontSize: 12, paddingTop: 10 }}
              iconType="circle"
              iconSize={8}
            />
          )}
          {series.map((s, i) => (
            <Line
              key={s.yKey}
              type="monotone"
              dataKey={s.yKey}
              name={s.name || s.yKey}
              stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
              strokeWidth={2}
              fill={s.fillGradient ? `url(#lineGradient-${i})` : 'none'}
              dot={{ fill: SERIES_COLORS[i % SERIES_COLORS.length], strokeWidth: 0, r: 3 }}
              activeDot={{ r: 5, fill: SERIES_COLORS[i % SERIES_COLORS.length] }}
            />
          ))}
        </RechartsLineChart>
      </ResponsiveContainer>
    </div>
  );
}

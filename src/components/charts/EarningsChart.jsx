import { useState } from 'react';
import { AreaChart as RechartsAreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer, BarChart, Bar, Cell } from 'recharts';

const colors = {
  primary:    '#f5a524',   /* --color-amber */
  secondary:  '#7b61ff',   /* --color-accent */
  tertiary:   '#5b9dff',   /* --color-info */
  quaternary: '#3ddc84',   /* --color-success */
  quinary:    '#a78bfa',   /* violet */
  grid:       '#23232e',   /* --color-border */
  text:       '#6b6b78',   /* --color-text-muted */
  background: '#111319',   /* --color-bg-card */
  border:     '#23232e',   /* --color-border */
};

const GAME_COLORS = [colors.primary, colors.secondary, colors.tertiary, colors.quaternary, colors.quinary, '#f472b6'];

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
        <p key={index} style={{ color: entry.color || entry.fill, margin: '4px 0', fontSize: 13 }}>
          {entry.name}: {typeof entry.value === 'number' ? `$${entry.value.toLocaleString()}` : entry.value}
        </p>
      ))}
    </div>
  );
}

function formatDate(value) {
  if (!value) return '';
  const date = new Date(value);
  if (isNaN(date.getTime())) return value;
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

function aggregateByGame(data) {
  const gameTotals = {};
  data.forEach(item => {
    if (!gameTotals[item.gameId]) {
      gameTotals[item.gameId] = {
        gameId: item.gameId,
        gameName: item.gameName || item.gameId,
        totalEarnings: 0,
      };
    }
    gameTotals[item.gameId].totalEarnings += item.earnings || 0;
  });
  return Object.values(gameTotals)
    .sort((a, b) => b.totalEarnings - a.totalEarnings)
    .slice(0, 6);
}

function aggregateByPeriod(data, period = 'daily') {
  const result = {};
  data.forEach(item => {
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

    if (!result[key]) {
      result[key] = { date: key };
    }
    result[key][item.gameId] = (result[key][item.gameId] || 0) + (item.earnings || 0);
  });

  return Object.values(result).sort((a, b) => new Date(a.date) - new Date(b.date));
}

export default function EarningsChart({
  data = [],
  title = 'Earnings Over Time',
  height = 350,
}) {
  const [view, setView] = useState('stacked');
  const [period, setPeriod] = useState('daily');

  const games = [...new Set(data.map(d => d.gameId))];
  const gameColors = {};
  games.forEach((gameId, i) => {
    gameColors[gameId] = GAME_COLORS[i % GAME_COLORS.length];
  });

  const aggregatedData = aggregateByPeriod(data, period);
  const gameTotals = aggregateByGame(data);

  const topGame = gameTotals[0];
  const totalEarnings = data.reduce((sum, d) => sum + (d.earnings || 0), 0);

  const lastPeriodData = aggregatedData.slice(-7);
  const prevPeriodData = aggregatedData.slice(-14, -7);
  const lastPeriodTotal = lastPeriodData.reduce((sum, d) => {
    return sum + games.reduce((s, g) => s + (d[g] || 0), 0);
  }, 0);
  const prevPeriodTotal = prevPeriodData.reduce((sum, d) => {
    return sum + games.reduce((s, g) => s + (d[g] || 0), 0);
  }, 0);
  const change = prevPeriodTotal > 0 ? (((lastPeriodTotal - prevPeriodTotal) / prevPeriodTotal) * 100).toFixed(1) : null;

  const renderChart = () => {
    if (view === 'stacked') {
      return (
        <ResponsiveContainer width="100%" height={height}>
          <RechartsAreaChart data={aggregatedData} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
            <defs>
              {games.map((gameId, i) => (
                <linearGradient key={gameId} id={`earningsGradient-${gameId}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor={GAME_COLORS[i % GAME_COLORS.length]} stopOpacity={0.4} />
                  <stop offset="95%" stopColor={GAME_COLORS[i % GAME_COLORS.length]} stopOpacity={0.05} />
                </linearGradient>
              ))}
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
            <XAxis
              dataKey="date"
              stroke={colors.text}
              tick={{ fill: colors.text, fontSize: 12 }}
              tickFormatter={(v) => formatDate(v)}
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
            {games.map(gameId => (
              <Area
                key={gameId}
                type="monotone"
                dataKey={gameId}
                name={gameColors[gameId] ? data.find(d => d.gameId === gameId)?.gameName || gameId : gameId}
                stackId="earnings"
                stroke={gameColors[gameId]}
                strokeWidth={1.5}
                fill={`url(#earningsGradient-${gameId})`}
              />
            ))}
          </RechartsAreaChart>
        </ResponsiveContainer>
      );
    }

    return (
      <ResponsiveContainer width="100%" height={height}>
        <BarChart
          data={gameTotals.map(g => ({ ...g, gameName: g.gameName || g.gameId }))}
          layout="vertical"
          margin={{ top: 10, right: 30, left: 80, bottom: 10 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
          <XAxis
            type="number"
            stroke={colors.text}
            tick={{ fill: colors.text, fontSize: 12 }}
            tickFormatter={(v) => `$${v >= 1000 ? `${(v/1000).toFixed(0)}k` : v}`}
          />
          <YAxis
            type="category"
            dataKey="gameName"
            stroke={colors.text}
            tick={{ fill: colors.text, fontSize: 12 }}
            width={80}
          />
          <Tooltip content={<CustomTooltip />} cursor={{ fill: `${colors.primary}20` }} />
          <Bar dataKey="totalEarnings" name="Total Earnings" radius={[0, 4, 4, 0]}>
            {gameTotals.map((entry, index) => (
              <Cell key={entry.gameId} fill={GAME_COLORS[index % GAME_COLORS.length]} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    );
  };

  return (
    <div className="chart-container">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        {title && <h4 className="chart-title" style={{ margin: 0 }}>{title}</h4>}
        <div style={{ display: 'flex', gap: 8 }}>
          <div style={{ display: 'flex', gap: 4 }}>
            {['stacked', 'compare'].map(v => (
              <button
                key={v}
                onClick={() => setView(v)}
                style={{
                  padding: '6px 12px',
                  borderRadius: 6,
                  border: '1px solid',
                  borderColor: view === v ? colors.primary : colors.border,
                  background: view === v ? `${colors.primary}20` : 'transparent',
                  color: view === v ? colors.primary : colors.text,
                  fontSize: 12,
                  cursor: 'pointer',
                  textTransform: 'capitalize',
                }}
              >
                {v}
              </button>
            ))}
          </div>
          {view === 'stacked' && (
            <div style={{ display: 'flex', gap: 4 }}>
              {['daily', 'weekly', 'monthly'].map(p => (
                <button
                  key={p}
                  onClick={() => setPeriod(p)}
                  style={{
                    padding: '6px 12px',
                    borderRadius: 6,
                    border: '1px solid',
                    borderColor: period === p ? colors.tertiary : colors.border,
                    background: period === p ? `${colors.tertiary}20` : 'transparent',
                    color: period === p ? colors.tertiary : colors.text,
                    fontSize: 12,
                    cursor: 'pointer',
                    textTransform: 'capitalize',
                  }}
                >
                  {p}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      <div style={{ display: 'flex', gap: 24, marginBottom: 20 }}>
        <div>
          <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>Total Earnings</p>
          <p style={{ color: colors.primary, fontSize: 20, fontWeight: 600, margin: '4px 0' }}>
            ${totalEarnings.toLocaleString()}
          </p>
          {change && (
            <p style={{
              color: parseFloat(change) >= 0 ? colors.tertiary : '#ef4444',
              fontSize: 12,
              margin: 0,
            }}>
              {parseFloat(change) >= 0 ? '+' : ''}{change}% vs prev period
            </p>
          )}
        </div>
        {topGame && (
          <div>
            <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>Top Game</p>
            <p style={{ color: colors.secondary, fontSize: 20, fontWeight: 600, margin: '4px 0' }}>
              {topGame.gameName}
            </p>
            <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>
              ${topGame.totalEarnings.toLocaleString()}
            </p>
          </div>
        )}
      </div>

      {renderChart()}
    </div>
  );
}

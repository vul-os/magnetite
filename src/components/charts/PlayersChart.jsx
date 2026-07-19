import { useState } from 'react';
import { AreaChart as RechartsAreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, BarChart, Bar } from 'recharts';

const colors = {
  primary:    '#f5a524',   /* --color-amber */
  secondary:  '#5b9dff',   /* --color-info */
  tertiary:   '#3ddc84',   /* --color-success */
  quaternary: '#a78bfa',   /* violet */
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
          {entry.name}: {typeof entry.value === 'number' ? entry.value.toLocaleString() : entry.value}
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

function calculateRetentionData(registrations) {
  const retentionBuckets = [0, 1, 7, 14, 30, 60, 90];
  return retentionBuckets.map(days => {
    const retained = registrations.filter(reg => {
      const regDate = new Date(reg.date);
      const now = new Date();
      const diffDays = Math.floor((now - regDate) / (1000 * 60 * 60 * 24));
      return diffDays >= days;
    }).length;
    const retentionRate = registrations.length > 0 ? (retained / registrations.length * 100) : 0;
    return {
      day: `Day ${days}`,
      retention: parseFloat(retentionRate.toFixed(1)),
    };
  });
}

export default function PlayersChart({
  data = {
    dailyActive: [],
    registrations: [],
  },
  title = 'Player Analytics',
  height = 300,
}) {
  const [view, setView] = useState('active');

  const dailyActive = data.dailyActive || [];
  const registrations = data.registrations || [];
  const retentionData = calculateRetentionData(registrations);

  const last7Days = dailyActive.slice(-7).reduce((sum, d) => sum + (d.users || 0), 0);
  const prev7Days = dailyActive.slice(-14, -7).reduce((sum, d) => sum + (d.users || 0), 0);
  const activeChange = prev7Days > 0 ? (((last7Days - prev7Days) / prev7Days) * 100).toFixed(1) : null;

  const last7Regs = registrations.slice(-7).reduce((sum, d) => sum + (d.newUsers || 0), 0);
  const prev7Regs = registrations.slice(-14, -7).reduce((sum, d) => sum + (d.newUsers || 0), 0);
  const regChange = prev7Regs > 0 ? (((last7Regs - prev7Regs) / prev7Regs) * 100).toFixed(1) : null;

  return (
    <div className="chart-container">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        {title && <h4 className="chart-title" style={{ margin: 0 }}>{title}</h4>}
        <div style={{ display: 'flex', gap: 8 }}>
          {[
            { key: 'active', label: 'Daily Active' },
            { key: 'new', label: 'New Users' },
            { key: 'retention', label: 'Retention' },
          ].map(v => (
            <button
              key={v.key}
              onClick={() => setView(v.key)}
              style={{
                padding: '6px 12px',
                borderRadius: 6,
                border: '1px solid',
                borderColor: view === v.key ? colors.primary : colors.border,
                background: view === v.key ? `${colors.primary}20` : 'transparent',
                color: view === v.key ? colors.primary : colors.text,
                fontSize: 12,
                cursor: 'pointer',
              }}
            >
              {v.label}
            </button>
          ))}
        </div>
      </div>

      <div style={{ display: 'flex', gap: 24, marginBottom: 20 }}>
        <div>
          <p style={{ color: colors.text, fontSize: 12, margin: 0 }}>
            {view === 'active' ? 'Daily Active Users (7d)' : view === 'new' ? 'New Registrations (7d)' : 'Retention Rate'}
          </p>
          <p style={{
            color: view === 'retention' ? colors.tertiary : colors.primary,
            fontSize: 20,
            fontWeight: 600,
            margin: '4px 0',
          }}>
            {view === 'active' ? last7Days.toLocaleString() : view === 'new' ? last7Regs.toLocaleString() : `${retentionData[0]?.retention || 0}%`}
          </p>
          {view !== 'retention' && (
            <p style={{
              color: parseFloat(view === 'active' ? activeChange : regChange) >= 0 ? colors.tertiary : '#ef4444',
              fontSize: 12,
              margin: 0,
            }}>
              {view === 'active' && activeChange && (parseFloat(activeChange) >= 0 ? '+' : '') + activeChange + '% vs prev'}
              {view === 'new' && regChange && (parseFloat(regChange) >= 0 ? '+' : '') + regChange + '% vs prev'}
            </p>
          )}
        </div>
      </div>

      {view === 'active' && (
        <ResponsiveContainer width="100%" height={height}>
          <RechartsAreaChart data={dailyActive} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
            <defs>
              <linearGradient id="activeUsersGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={colors.primary} stopOpacity={0.4} />
                <stop offset="95%" stopColor={colors.primary} stopOpacity={0.05} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
            <XAxis
              dataKey="date"
              stroke={colors.text}
              tick={{ fill: colors.text, fontSize: 12 }}
              tickFormatter={formatDate}
            />
            <YAxis stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
            <Tooltip content={<CustomTooltip />} cursor={{ stroke: colors.grid, strokeWidth: 1 }} />
            <Area
              type="monotone"
              dataKey="users"
              name="Active Users"
              stroke={colors.primary}
              strokeWidth={2}
              fill="url(#activeUsersGradient)"
            />
          </RechartsAreaChart>
        </ResponsiveContainer>
      )}

      {view === 'new' && (
        <ResponsiveContainer width="100%" height={height}>
          <BarChart data={registrations.slice(-14)} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
            <XAxis
              dataKey="date"
              stroke={colors.text}
              tick={{ fill: colors.text, fontSize: 12 }}
              tickFormatter={formatDate}
            />
            <YAxis stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
            <Tooltip content={<CustomTooltip />} cursor={{ fill: `${colors.secondary}20` }} />
            <Bar dataKey="newUsers" name="New Users" fill={colors.secondary} radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      )}

      {view === 'retention' && (
        <ResponsiveContainer width="100%" height={height}>
          <RechartsAreaChart data={retentionData} margin={{ top: 10, right: 30, left: 0, bottom: 10 }}>
            <defs>
              <linearGradient id="retentionGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={colors.tertiary} stopOpacity={0.4} />
                <stop offset="95%" stopColor={colors.tertiary} stopOpacity={0.05} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
            <XAxis dataKey="day" stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
            <YAxis
              stroke={colors.text}
              tick={{ fill: colors.text, fontSize: 12 }}
              tickFormatter={(v) => `${v}%`}
              domain={[0, 100]}
            />
            <Tooltip
              content={<CustomTooltip />}
              cursor={{ stroke: colors.grid, strokeWidth: 1 }}
              formatter={(value) => [`${value}%`, 'Retention']}
            />
            <Area
              type="monotone"
              dataKey="retention"
              name="Retention Rate"
              stroke={colors.tertiary}
              strokeWidth={2}
              fill="url(#retentionGradient)"
            />
          </RechartsAreaChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}

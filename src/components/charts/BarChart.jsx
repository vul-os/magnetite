import { BarChart as RechartsBarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

const colors = {
  primary:    '#f5a524',   /* --color-amber */
  secondary:  '#7b61ff',   /* --color-accent */
  grid:       '#23232e',   /* --color-border */
  text:       '#6b6b78',   /* --color-text-muted */
  background: '#111319',   /* --color-bg-card */
};

export default function BarChart({ data, xKey, yKey, title, horizontal = false }) {
  return (
    <div className="chart-container">
      {title && <h4 className="chart-title">{title}</h4>}
      <ResponsiveContainer width="100%" height={300}>
        <RechartsBarChart data={data} layout={horizontal ? 'vertical' : 'horizontal'} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke={colors.grid} />
          {horizontal ? (
            <>
              <XAxis type="number" stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
              <YAxis dataKey={xKey} type="category" stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} width={80} />
            </>
          ) : (
            <>
              <XAxis dataKey={xKey} stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
              <YAxis stroke={colors.text} tick={{ fill: colors.text, fontSize: 12 }} />
            </>
          )}
          <Tooltip
            contentStyle={{ background: colors.background, border: `1px solid ${colors.grid}`, borderRadius: 8 }}
            labelStyle={{ color: '#e4e4e7' }}
            itemStyle={{ color: colors.primary }}
          />
          <Bar dataKey={yKey} fill={colors.primary} radius={[4, 4, 0, 0]} />
        </RechartsBarChart>
      </ResponsiveContainer>
    </div>
  );
}

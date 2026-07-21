/**
 * AnalyticsChart — single-series time-series chart for the Developer Analytics
 * page. Renders either revenue-over-time (USD) or playtime-over-time (minutes)
 * from a flat [{ date, value }] series.
 *
 * Themed via src/styles/tokens.css, not hardcoded. Recharts needs literal
 * colour strings (SVG attributes don't resolve CSS custom properties), so this
 * component reads the *computed* token values off <html> and re-reads them
 * whenever data-theme flips — the same mechanism ThemeContext uses to publish
 * the theme. It must never fall back to a baked-in dark palette, or the chart
 * silently stops following the user's theme (the bug this replaced).
 */
import { useEffect, useMemo, useState } from 'react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

const MONO_STACK = "'IBM Plex Mono', ui-monospace, 'SF Mono', Menlo, Consolas, monospace";

/** One series identity per the four-token colour system in DESIGN.md §2. */
const SERIES_VAR = {
  amber: '--spec',
  field: '--field',
};

/** Read the live computed value of a CSS custom property off <html>. */
function readVar(name, fallback) {
  if (typeof window === 'undefined') return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}

/**
 * Re-render whenever data-theme changes on <html>, so recharts (which needs
 * literal colour strings) stays in sync with the token layer instead of
 * freezing at whatever theme was active on first mount.
 */
function useThemeTick() {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver((mutations) => {
      if (mutations.some((m) => m.attributeName === 'data-theme')) {
        setTick((t) => t + 1);
      }
    });
    observer.observe(root, { attributes: true, attributeFilter: ['data-theme'] });
    return () => observer.disconnect();
  }, []);
  return tick;
}

function useChartTokens() {
  const tick = useThemeTick();
  return useMemo(() => ({
    amber:  readVar(SERIES_VAR.amber, '#FFB020'),
    field:  readVar(SERIES_VAR.field, '#8B74FF'),
    grid:   readVar('--line', '#1C212D'),
    text:   readVar('--ink-3', '#6B7488'),
    bg:     readVar('--elevated', '#161A24'),
    border: readVar('--line-2', '#2E3646'),
    ink:    readVar('--ink', '#E7EAF2'),
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }), [tick]);
}

/* ── Custom tooltip ──────────────────────────────────────────────────────── */
function ChartTooltip({ active, payload, label, formatter, tokens }) {
  if (!active || !payload || !payload.length) return null;
  const raw = payload[0]?.value ?? 0;
  return (
    <div style={{
      background: tokens.bg,
      border: `1px solid ${tokens.border}`,
      borderRadius: 8,
      padding: '10px 14px',
      boxShadow: '0 8px 24px rgba(0,0,0,0.35)',
    }}>
      <p style={{ margin: '0 0 6px', fontFamily: MONO_STACK, fontSize: 11, color: tokens.text, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
        {label}
      </p>
      <p style={{ margin: 0, fontFamily: MONO_STACK, fontSize: 17, fontWeight: 700, color: payload[0]?.color ?? tokens.amber }}>
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
  color = 'amber',     // 'amber' (--spec) | 'field' (--field) — DESIGN.md §2 accents
  gradientId,          // unique id for the SVG gradient — must be unique per page
  yFormatter,          // (value) => string
  tooltipFormatter,    // (value) => string
  height = 260,
  emptyMessage = 'No data for this period.',
}) {
  const tokens = useChartTokens();
  const stroke = color === 'field' ? tokens.field : tokens.amber;
  const gId = gradientId ?? `grad-${color}`;

  if (!data || data.length === 0) {
    return (
      <div style={{ height, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <p style={{ fontFamily: MONO_STACK, fontSize: 13, color: tokens.text }}>
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
        <CartesianGrid strokeDasharray="3 3" stroke={tokens.grid} vertical={false} />
        <XAxis
          dataKey="date"
          stroke={tokens.text}
          tick={{ fill: tokens.text, fontSize: 11, fontFamily: MONO_STACK }}
          tickLine={false}
          axisLine={false}
          tickFormatter={shortDate}
          interval="preserveStartEnd"
        />
        <YAxis
          stroke={tokens.text}
          tick={{ fill: tokens.text, fontSize: 11, fontFamily: MONO_STACK }}
          tickLine={false}
          axisLine={false}
          tickFormatter={yFormatter ?? ((v) => v)}
          width={56}
        />
        <Tooltip
          content={<ChartTooltip formatter={tooltipFormatter} tokens={tokens} />}
          cursor={{ stroke: tokens.grid, strokeWidth: 1 }}
        />
        <Area
          type="monotone"
          dataKey="value"
          stroke={stroke}
          strokeWidth={2}
          fill={`url(#${gId})`}
          dot={false}
          activeDot={{ r: 4, fill: stroke, strokeWidth: 0 }}
          isAnimationActive={typeof window !== 'undefined' && !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}

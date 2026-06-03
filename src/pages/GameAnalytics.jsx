/**
 * GameAnalytics — per-game developer analytics dashboard.
 * Route: /developers/analytics/:gameId
 *
 * Displays:
 *  • KPI cards: total revenue, active players, total sessions
 *  • Revenue-over-time chart (30-day daily_revenue)
 *  • Playtime-over-time chart (30-day daily_playtime)
 *  • Date-range selector (7d / 14d / 30d / all)
 *  • Per-game breakdown table (shows stats for all developer games)
 *
 * Wired to: GET /api/v1/developer/games/:id/analytics
 * Mock: enabled when VITE_USE_MOCKS === 'true'
 */
import { useState, useEffect, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import Layout from '../components/Layout';
import { api } from '../api/client';
import AnalyticsChart from '../components/charts/AnalyticsChart';
import { useTranslation } from '../i18n/useTranslation';
import './GameAnalytics.css';

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

/* ── Mock data ─────────────────────────────────────────────────────────────── */
function buildMockSeries(days, baseRevenue, variance) {
  const out = [];
  const now = new Date('2026-06-01');
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(now);
    d.setDate(d.getDate() - i);
    out.push({
      date: d.toISOString().split('T')[0],
      value: Math.max(0, baseRevenue + (Math.random() - 0.45) * variance),
    });
  }
  return out;
}

const MOCK_ANALYTICS = {
  game_id: '1',
  game_title: 'Cosmic Raiders',
  summary: {
    total_revenue: 12450.0,
    active_players: 8420,
    total_sessions: 54203,
  },
  daily_revenue: buildMockSeries(30, 415, 180),
  daily_playtime: buildMockSeries(30, 12400, 3500),
};

const MOCK_GAMES_BREAKDOWN = [
  { id: 1, title: 'Cosmic Raiders',   revenue: 12450.0,  players: 8420, sessions: 54203 },
  { id: 2, title: 'Galaxy Conquest',  revenue: 2970.5,   players: 2941, sessions: 14201 },
  { id: 3, title: 'Dungeon Realms',   revenue: 7840.0,   players: 3240, sessions: 28400 },
  { id: 4, title: 'Neon Drift',       revenue: 1320.0,   players:  820, sessions:  4100 },
];

/* ── Date-range helpers ──────────────────────────────────────────────────── */
const RANGES = [
  { label: '7d',  days: 7 },
  { label: '14d', days: 14 },
  { label: '30d', days: 30 },
  { label: 'All', days: null },
];

function sliceSeries(series, days) {
  if (!days || !series) return series ?? [];
  return series.slice(-days);
}

function sum(series) {
  return series.reduce((acc, d) => acc + (d.value ?? 0), 0);
}

/* ── KPI Card ────────────────────────────────────────────────────────────── */
function KpiCard({ label, value, sub, accent }) {
  return (
    <div className={`analytics-kpi-card${accent ? ' accent' : ''}`}>
      <span className="kpi-label">{label}</span>
      <span className={`kpi-value${accent ? ' amber' : ''}`}>{value}</span>
      {sub && <span className="kpi-sub">{sub}</span>}
    </div>
  );
}

/* ── Loading skeleton ────────────────────────────────────────────────────── */
function Skeleton({ h = 260 }) {
  return (
    <div
      className="analytics-skeleton"
      style={{ height: h }}
      aria-hidden="true"
    />
  );
}

/* ── Main component ──────────────────────────────────────────────────────── */
export default function GameAnalytics() {
  const { t } = useTranslation();
  const { gameId } = useParams();

  const [analytics, setAnalytics] = useState(USE_MOCKS ? MOCK_ANALYTICS : null);
  const [gamesBreakdown, setGamesBreakdown] = useState(USE_MOCKS ? MOCK_GAMES_BREAKDOWN : []);
  const [loading, setLoading] = useState(!USE_MOCKS);
  const [loadError, setLoadError] = useState(null);
  const [range, setRange] = useState(2); // index into RANGES → default 30d

  const loadData = useCallback(async () => {
    if (USE_MOCKS) return;
    setLoading(true);
    setLoadError(null);
    try {
      const [analyticsRes, gamesRes] = await Promise.allSettled([
        api.developer.gameAnalytics(gameId),
        api.developer.games(),
      ]);

      if (analyticsRes.status === 'fulfilled') {
        const raw = analyticsRes.value?.data ?? analyticsRes.value;
        // Normalise daily_revenue → [{ date, value }]
        const daily_revenue = (raw?.daily_revenue ?? []).map(p => ({
          date: p.date,
          value: Number(p.revenue ?? p.value ?? 0),
        }));
        // Normalise daily_playtime → [{ date, value }]
        const daily_playtime = (raw?.daily_playtime ?? []).map(p => ({
          date: p.date,
          value: Number(p.minutes ?? p.playtime_minutes ?? p.value ?? 0),
        }));
        setAnalytics({
          game_id: raw?.game_id ?? gameId,
          game_title: raw?.game_title ?? raw?.title ?? 'Game',
          summary: {
            total_revenue:  Number(raw?.summary?.total_revenue  ?? 0),
            active_players: Number(raw?.summary?.active_players ?? 0),
            total_sessions: Number(raw?.summary?.total_sessions ?? 0),
          },
          daily_revenue,
          daily_playtime,
        });
      } else {
        throw analyticsRes.reason;
      }

      if (gamesRes.status === 'fulfilled') {
        const d = gamesRes.value?.data ?? gamesRes.value;
        const list = Array.isArray(d) ? d : (d?.games ?? []);
        setGamesBreakdown(list.map(g => ({
          id: g.id,
          title: g.title,
          revenue: Number(g.total_revenue ?? g.earnings ?? 0),
          players: Number(g.total_players ?? g.players ?? 0),
          sessions: Number(g.total_sessions ?? g.sessions ?? 0),
        })));
      }
    } catch (err) {
      setLoadError(err.message || t('analytics.loadError'));
    } finally {
      setLoading(false);
    }
  }, [gameId, t]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const days = RANGES[range].days;
  const revSeries = sliceSeries(analytics?.daily_revenue, days);
  const playSeries = sliceSeries(analytics?.daily_playtime, days);

  /* Derived KPIs from visible window (if range is filtered, show window sum) */
  const windowRevenue = revSeries.length ? sum(revSeries) : (analytics?.summary?.total_revenue ?? 0);
  const windowMinutes = playSeries.length ? sum(playSeries) : 0;
  const fmtUsd = (v) => `$${Number(v).toLocaleString(undefined, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}`;
  const fmtMin = (v) => `${Math.round(v).toLocaleString()} ${t('analytics.min')}`;

  return (
    <Layout>
      <div className="game-analytics">
        {/* ── Header ──────────────────────────────────────────────────── */}
        <header className="analytics-header">
          <nav className="analytics-breadcrumb" aria-label={t('analytics.breadcrumbLabel')}>
            <Link to="/developers" className="breadcrumb-link">{t('analytics.dashboard')}</Link>
            <span className="breadcrumb-sep" aria-hidden="true">/</span>
            <span className="breadcrumb-current" aria-current="page">
              {analytics?.game_title ?? (loading ? t('common.loading') : t('analytics.title'))}
            </span>
          </nav>
          <div className="analytics-header-row">
            <div>
              <span className="kicker">// {t('analytics.kicker')}</span>
              <h1>{analytics?.game_title ?? (loading ? '—' : t('analytics.title'))}</h1>
              <p>{t('analytics.subtitle')}</p>
            </div>
            <div className="range-selector" role="group" aria-label={t('analytics.dateRange')}>
              {RANGES.map((r, i) => (
                <button
                  key={r.label}
                  className={`range-btn${range === i ? ' active' : ''}`}
                  onClick={() => setRange(i)}
                  aria-pressed={range === i}
                  aria-label={t('analytics.rangeLabel', { range: r.label })}
                >
                  {r.label}
                </button>
              ))}
            </div>
          </div>
        </header>

        {/* ── Error banner ────────────────────────────────────────────── */}
        {loadError && (
          <div role="alert" className="analytics-error">
            <span>{loadError}</span>
            <button className="analytics-retry" onClick={loadData} aria-label={t('common.retry')}>
              {t('common.retry')}
            </button>
          </div>
        )}

        {/* ── KPI cards ───────────────────────────────────────────────── */}
        <section className="analytics-kpi-grid" aria-label={t('analytics.kpiLabel')}>
          <KpiCard
            label={t('analytics.revenueKpi', { range: RANGES[range].label })}
            value={loading ? '—' : fmtUsd(windowRevenue)}
            sub={t('analytics.usdGross')}
            accent
          />
          <KpiCard
            label={t('analytics.activePlayers')}
            value={loading ? '—' : (analytics?.summary?.active_players ?? 0).toLocaleString()}
            sub={t('analytics.uniquePlayers')}
          />
          <KpiCard
            label={t('analytics.totalSessions')}
            value={loading ? '—' : (analytics?.summary?.total_sessions ?? 0).toLocaleString()}
            sub={t('analytics.allTime')}
          />
          <KpiCard
            label={t('analytics.playtimeKpi', { range: RANGES[range].label })}
            value={loading ? '—' : fmtMin(windowMinutes)}
            sub={t('analytics.minutesPlayed')}
          />
        </section>

        {/* ── Charts row ──────────────────────────────────────────────── */}
        <section className="analytics-charts-grid" aria-label={t('analytics.chartsLabel')}>
          {/* Revenue chart */}
          <div className="analytics-card">
            <div className="analytics-card-header">
              <div>
                <span className="kicker">// {t('analytics.revenueKicker')}</span>
                <h2>{t('analytics.revenueChart')}</h2>
              </div>
              <span className="chart-badge amber">USD</span>
            </div>
            {loading ? (
              <Skeleton />
            ) : (
              <AnalyticsChart
                data={revSeries}
                color="amber"
                gradientId="revGrad"
                yFormatter={(v) => v >= 1000 ? `$${(v / 1000).toFixed(0)}k` : `$${v}`}
                tooltipFormatter={fmtUsd}
                emptyMessage={t('analytics.noRevenueData')}
              />
            )}
          </div>

          {/* Playtime chart */}
          <div className="analytics-card">
            <div className="analytics-card-header">
              <div>
                <span className="kicker">// {t('analytics.engagementKicker')}</span>
                <h2>{t('analytics.playtimeChart')}</h2>
              </div>
              <span className="chart-badge cyan">{t('analytics.min')}</span>
            </div>
            {loading ? (
              <Skeleton />
            ) : (
              <AnalyticsChart
                data={playSeries}
                color="cyan"
                gradientId="playGrad"
                yFormatter={(v) => v >= 1000 ? `${(v / 1000).toFixed(0)}k` : String(Math.round(v))}
                tooltipFormatter={fmtMin}
                emptyMessage={t('analytics.noPlaytimeData')}
              />
            )}
          </div>
        </section>

        {/* ── Per-game breakdown table ─────────────────────────────────── */}
        <section className="analytics-breakdown" aria-label={t('analytics.breakdownLabel')}>
          <div className="analytics-card">
            <div className="analytics-card-header">
              <div>
                <span className="kicker">// {t('analytics.allGamesKicker')}</span>
                <h2>{t('analytics.breakdownTitle')}</h2>
              </div>
              <Link to="/developers" className="view-all-link">{t('analytics.backToDashboard')}</Link>
            </div>

            {loading ? (
              <Skeleton h={120} />
            ) : gamesBreakdown.length === 0 ? (
              <div className="analytics-empty">
                <p>{t('analytics.noGamesData')}</p>
                <Link to="/game-studio" className="btn btn-primary" style={{ marginTop: '1rem', display: 'inline-block' }}>
                  {t('analytics.createFirstGame')}
                </Link>
              </div>
            ) : (
              <div className="breakdown-table-wrapper">
                <table className="breakdown-table" aria-label={t('analytics.breakdownTableLabel')}>
                  <thead>
                    <tr>
                      <th scope="col">{t('analytics.colGame')}</th>
                      <th scope="col">{t('analytics.colRevenue')}</th>
                      <th scope="col">{t('analytics.colActivePlayers')}</th>
                      <th scope="col">{t('analytics.colSessions')}</th>
                      <th scope="col">{t('analytics.colView')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gamesBreakdown.map(g => (
                      <tr
                        key={g.id}
                        className={String(g.id) === String(gameId) ? 'row-active' : ''}
                        aria-current={String(g.id) === String(gameId) ? 'true' : undefined}
                      >
                        <td className="game-name-cell">
                          {String(g.id) === String(gameId) && (
                            <span className="current-badge" aria-label={t('analytics.currentGame')}>&#9654;</span>
                          )}
                          {g.title}
                        </td>
                        <td className="rev-cell">{`$${Number(g.revenue).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`}</td>
                        <td className="num-cell">{Number(g.players).toLocaleString()}</td>
                        <td className="num-cell">{Number(g.sessions).toLocaleString()}</td>
                        <td>
                          <Link
                            to={`/developers/analytics/${g.id}`}
                            className={`breakdown-link${String(g.id) === String(gameId) ? ' active' : ''}`}
                            aria-label={t('analytics.viewGameAnalytics', { title: g.title })}
                          >
                            {t('analytics.view')}
                          </Link>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </section>
      </div>
    </Layout>
  );
}

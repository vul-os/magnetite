import { useState } from 'react';
import Layout from '../components/Layout';
import Skeleton from '../components/skeletons/Skeleton';
import EmptyState from '../components/empty/EmptyState';
import { usePoints } from '../hooks/usePoints';
import './Points.css';

// ── Icons ─────────────────────────────────────────────────────────────────────

function StarIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <polygon points="12,2 15.09,8.26 22,9.27 17,14.14 18.18,21.02 12,17.77 5.82,21.02 7,14.14 2,9.27 8.91,8.26" />
    </svg>
  );
}

function HistoryIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <polyline points="1,4 1,10 7,10" />
      <path d="M3.51 15a9 9 0 1 0 .49-4.95" />
    </svg>
  );
}

function GiftIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <polyline points="20,12 20,22 4,22 4,12" />
      <rect x="2" y="7" width="20" height="5" />
      <line x1="12" y1="22" x2="12" y2="7" />
      <path d="M12 7H7.5a2.5 2.5 0 0 1 0-5C11 2 12 7 12 7z" />
      <path d="M12 7h4.5a2.5 2.5 0 0 0 0-5C13 2 12 7 12 7z" />
    </svg>
  );
}

function TrophySmIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
      <path d="M8 21h8M12 17v4M7 4H4a2 2 0 0 0-2 2v2c0 2.2 1.8 4 4 4h1M17 4h3a2 2 0 0 1 2 2v2c0 2.2-1.8 4-4 4h-1M7 4v8a5 5 0 0 0 10 0V4" />
    </svg>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Map tier names to CSS variable tokens (no hardcoded hex).
const TIER_CSS_VARS = {
  Bronze:   'var(--color-tier-bronze,  #cd7f32)',
  Silver:   'var(--color-tier-silver,  var(--color-text-secondary))',
  Gold:     'var(--color-tier-gold,    var(--color-amber))',
  Platinum: 'var(--color-tier-platinum,var(--color-accent))',
  Diamond:  'var(--color-tier-diamond, var(--color-info))',
};
// Alias kept as TIER_COLORS so all downstream references remain valid.
const TIER_COLORS = TIER_CSS_VARS;

function formatDate(iso) {
  return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

function formatPts(n) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}

const TABS = [
  { key: 'overview',    label: 'Overview',    icon: StarIcon    },
  { key: 'history',     label: 'History',     icon: HistoryIcon },
  { key: 'rewards',     label: 'Rewards',     icon: GiftIcon    },
  { key: 'leaderboard', label: 'Leaderboard', icon: TrophySmIcon },
];

// ─────────────────────────────────────────────────────────────────────────────

export default function Points() {
  const [tab, setTab]                 = useState('overview');
  const [redeemMsg, setRedeemMsg]     = useState(null);
  const { balance, history, rewards, leaderboard, loading, redeeming, redeem } = usePoints();

  async function handleRedeem(reward) {
    if (balance.points < reward.cost) {
      setRedeemMsg({ type: 'error', text: 'Not enough points.' });
      setTimeout(() => setRedeemMsg(null), 3000);
      return;
    }
    const res = await redeem(reward.id);
    setRedeemMsg(res.success
      ? { type: 'success', text: `Redeemed "${reward.name}" successfully!` }
      : { type: 'error',   text: res.error || 'Redemption failed.' }
    );
    setTimeout(() => setRedeemMsg(null), 3500);
  }

  const season = balance.season ?? {};
  const tierColor = TIER_COLORS[season.tier] ?? 'var(--color-accent)';

  return (
    <Layout>
      <div className="points-page reveal">

        {/* ── Header ── */}
        <header className="points-header reveal-1">
          <span className="kicker">// Season Economy</span>
          <h1>Points &amp; Rewards</h1>
          <p className="points-subtitle">Earn points by playing, competing, and engaging. Redeem for cosmetics and boosts.</p>
        </header>

        {/* ── Balance hero ── */}
        <div className="points-balance-hero reveal-2" aria-label="Points balance">
          {loading ? (
            <Skeleton variant="rect" width="100%" height="120px" />
          ) : (
            <>
              <div className="points-hero-left">
                <span className="points-hero-label">Current Balance</span>
                <div className="points-hero-value">
                  <span className="points-coin" aria-hidden="true">⬡</span>
                  <span className="points-hero-number">{balance.points.toLocaleString()}</span>
                  <span className="points-hero-unit">pts</span>
                </div>
                <span className="points-hero-lifetime">
                  {balance.lifetime_points?.toLocaleString()} lifetime pts · Global #{balance.rank}
                </span>
              </div>
              <div className="points-season-card">
                <div className="season-tier-row">
                  <span className="season-tier-dot" style={{ background: tierColor }} aria-hidden="true" />
                  <span className="season-tier-name" style={{ color: tierColor }}>{season.tier}</span>
                  <span className="season-name">{season.name}</span>
                </div>
                <div className="season-progress-wrap">
                  <div className="season-progress-labels">
                    <span>{season.tier}</span>
                    <span>{season.next_tier}</span>
                  </div>
                  <div
                    className="season-bar-track"
                    role="progressbar"
                    aria-valuenow={season.progress}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label={`${season.progress}% toward ${season.next_tier}`}
                  >
                    <div className="season-bar-fill" style={{ width: `${season.progress}%` }} />
                  </div>
                  <span className="season-pts-needed">{season.points_needed?.toLocaleString()} pts to {season.next_tier}</span>
                </div>
                <span className="season-ends">Season ends {formatDate(season.ends_at)}</span>
              </div>
            </>
          )}
        </div>

        {/* ── Toast ── */}
        {redeemMsg && (
          <div className={`points-toast points-toast-${redeemMsg.type}`} role="status" aria-live="polite">
            {redeemMsg.text}
          </div>
        )}

        {/* ── Tab bar ── */}
        <div className="points-tabs reveal-3" role="tablist" aria-label="Points sections">
          {TABS.map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              role="tab"
              className={`points-tab${tab === key ? ' active' : ''}`}
              onClick={() => setTab(key)}
              aria-selected={tab === key}
              aria-controls={`points-panel-${key}`}
              id={`points-tab-${key}`}
            >
              <span className="points-tab-icon" aria-hidden="true"><Icon /></span>
              {label}
            </button>
          ))}
        </div>

        {/* ── Overview ── */}
        {tab === 'overview' && (
          <section id="points-panel-overview" className="points-section reveal-4" role="tabpanel" aria-labelledby="points-tab-overview" aria-label="Points overview">
            <div className="points-stat-grid">
              {[
                { label: 'Balance',        value: formatPts(balance.points),           unit: 'pts'  },
                { label: 'Lifetime Earned', value: formatPts(balance.lifetime_points), unit: 'pts'  },
                { label: 'Global Rank',    value: `#${balance.rank}`,                  unit: ''     },
                { label: 'Season',         value: season.tier,                          unit: ''     },
              ].map(({ label, value, unit }) => (
                <div key={label} className="points-stat-card">
                  {loading ? (
                    <>
                      <Skeleton variant="text" width="50%" height="12px" />
                      <Skeleton variant="text" width="70%" height="28px" />
                    </>
                  ) : (
                    <>
                      <span className="stat-label">{label}</span>
                      <span className="stat-value">{value}<span className="stat-unit">{unit}</span></span>
                    </>
                  )}
                </div>
              ))}
            </div>

            <div className="points-how-earn">
              <h3 className="section-sub-heading">How to Earn Points</h3>
              <div className="earn-methods-grid">
                {[
                  { icon: '🎮', title: 'Play Games',           desc: 'Earn pts per session minute played.' },
                  { icon: '🏆', title: 'Win Tournaments',       desc: 'Placement rewards up to 5,000 pts.' },
                  { icon: '🔥', title: 'Daily Streak',          desc: '50–500 pts for daily logins.' },
                  { icon: '🤝', title: 'Refer Friends',         desc: '200 pts when a friend joins.' },
                  { icon: '⭐', title: 'Leave a Review',        desc: '50 pts per reviewed game.' },
                  { icon: '🎯', title: 'Unlock Achievements',   desc: 'Variable pts per achievement.' },
                ].map(({ icon, title, desc }) => (
                  <div key={title} className="earn-method-card">
                    <span className="earn-method-icon" aria-hidden="true">{icon}</span>
                    <div>
                      <span className="earn-method-title">{title}</span>
                      <span className="earn-method-desc">{desc}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </section>
        )}

        {/* ── History ── */}
        {tab === 'history' && (
          <section id="points-panel-history" className="points-section reveal-4" role="tabpanel" aria-labelledby="points-tab-history" aria-label="Points history">
            <h3 className="section-sub-heading">Transaction History</h3>
            {loading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className="points-tx-row">
                  <Skeleton variant="rect" width="36px" height="36px" />
                  <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '6px' }}>
                    <Skeleton variant="text" width="60%" height="13px" />
                    <Skeleton variant="text" width="30%" height="11px" />
                  </div>
                  <Skeleton variant="text" width="60px" height="16px" />
                </div>
              ))
            ) : history.length === 0 ? (
              <EmptyState
                icon={<HistoryIcon />}
                title="No transactions yet"
                description="Play games and complete activities to earn points."
              />
            ) : (
              <div className="points-tx-list" role="list">
                {history.map(tx => (
                  <div key={tx.id} className="points-tx-row" role="listitem">
                    <div className={`points-tx-icon points-tx-icon-${tx.type}`} aria-hidden="true">
                      {tx.type === 'earn' ? '+' : '−'}
                    </div>
                    <div className="points-tx-info">
                      <span className="points-tx-desc">{tx.description}</span>
                      <span className="points-tx-date">{formatDate(tx.created_at)}</span>
                    </div>
                    <span className={`points-tx-amount ${tx.amount > 0 ? 'positive' : 'negative'}`}>
                      {tx.amount > 0 ? '+' : ''}{tx.amount.toLocaleString()} pts
                    </span>
                  </div>
                ))}
              </div>
            )}
          </section>
        )}

        {/* ── Rewards ── */}
        {tab === 'rewards' && (
          <section id="points-panel-rewards" className="points-section reveal-4" role="tabpanel" aria-labelledby="points-tab-rewards" aria-label="Rewards shop">
            <h3 className="section-sub-heading">Redeem Rewards</h3>
            <p className="section-desc">Spend your points on cosmetics, boosts, and currency credits.</p>
            <div className="rewards-grid" role="list">
              {loading ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <div key={i} className="reward-card">
                    <Skeleton variant="rect" width="80px" height="80px" />
                    <Skeleton variant="text" width="80%" height="14px" />
                    <Skeleton variant="text" width="60%" height="12px" />
                    <Skeleton variant="rect" width="100%" height="36px" />
                  </div>
                ))
              ) : rewards.map(reward => {
                const canAfford = balance.points >= reward.cost;
                return (
                  <div key={reward.id} className={`reward-card${!reward.available ? ' reward-unavailable' : ''}`} role="listitem">
                    <img src={reward.image} alt="" className="reward-img" aria-hidden="true" width={80} height={80} />
                    <div className="reward-type-badge">{reward.type}</div>
                    <h4 className="reward-name">{reward.name}</h4>
                    <p className="reward-desc">{reward.description}</p>
                    <div className="reward-cost">
                      <span className="reward-cost-icon" aria-hidden="true">⬡</span>
                      <span>{reward.cost.toLocaleString()} pts</span>
                    </div>
                    <button
                      className={`btn ${canAfford && reward.available ? 'btn-primary' : 'btn-secondary'} reward-btn`}
                      onClick={() => handleRedeem(reward)}
                      disabled={!canAfford || !reward.available || redeeming}
                      aria-label={`Redeem ${reward.name} for ${reward.cost} points`}
                    >
                      {!reward.available ? 'Coming Soon' : canAfford ? 'Redeem' : 'Not Enough Points'}
                    </button>
                  </div>
                );
              })}
            </div>
          </section>
        )}

        {/* ── Leaderboard ── */}
        {tab === 'leaderboard' && (
          <section id="points-panel-leaderboard" className="points-section reveal-4" role="tabpanel" aria-labelledby="points-tab-leaderboard" aria-label="Points leaderboard">
            <h3 className="section-sub-heading">Top Point Earners</h3>
            <div
              className="points-lb-table"
              role="table"
              aria-label="Points leaderboard"
            >
              <div className="points-lb-header" role="row">
                <span role="columnheader">Rank</span>
                <span role="columnheader">Player</span>
                <span role="columnheader">Points</span>
              </div>
              {loading ? (
                Array.from({ length: 10 }).map((_, i) => (
                  <div key={i} className="points-lb-row" role="row">
                    <Skeleton variant="text" width="30px" height="13px" />
                    <Skeleton variant="text" width="120px" height="13px" />
                    <Skeleton variant="text" width="70px" height="13px" />
                  </div>
                ))
              ) : leaderboard.map((entry, idx) => (
                <div
                  key={entry.rank}
                  className={`points-lb-row${idx < 3 ? ` points-lb-top${idx + 1}` : ''}`}
                  role="row"
                >
                  <span className="lb-rank" role="cell">
                    {idx === 0 ? '🥇' : idx === 1 ? '🥈' : idx === 2 ? '🥉' : `#${entry.rank}`}
                  </span>
                  <span className="lb-player" role="cell">
                    <img src={entry.avatar} alt="" className="lb-avatar" width={24} height={24} />
                    {entry.username}
                  </span>
                  <span className="lb-pts" role="cell">{entry.points.toLocaleString()}</span>
                </div>
              ))}
            </div>
          </section>
        )}

      </div>
    </Layout>
  );
}

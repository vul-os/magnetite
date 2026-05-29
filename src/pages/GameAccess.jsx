import Layout from '../components/Layout';
import SubscriptionBadge from '../components/SubscriptionBadge';
import './GameAccess.css';

const MOCK_GAMES = [
  { id: 1, title: 'Cosmic Drift',  category: 'Racing',       tier: 'free',      players: 1247, image: 'https://images.unsplash.com/photo-1511882150382-421056c89033?w=400' },
  { id: 2, title: 'Neon Strike',   category: 'Action',       tier: 'basic',     players: 892,  image: 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?w=400' },
  { id: 3, title: 'Cyber Arena',   category: 'Battle Royale', tier: 'pro',      players: 3421, image: 'https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400' },
  { id: 4, title: 'Quantum Poker', category: 'Card Game',    tier: 'free',      players: 567,  image: 'https://images.unsplash.com/photo-1526304640581-d334cdbbf45e?w=400' },
  { id: 5, title: 'Void Hunters',  category: 'Adventure',    tier: 'pro',       players: 1823, image: 'https://images.unsplash.com/photo-1538481199705-c710c4e965fc?w=400' },
  { id: 6, title: 'Master Chess',  category: 'Strategy',     tier: 'unlimited', players: 4521, image: 'https://images.unsplash.com/photo-1529699211952-734e80c4d42b?w=400' },
];

const TIER_LABELS = {
  free:      'Free Access',
  basic:     'Basic Tier',
  pro:       'Pro Tier',
  unlimited: 'Unlimited',
};

const TIER_ORDER = ['free', 'basic', 'pro', 'unlimited'];

function LockIcon({ className }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </svg>
  );
}

function PlayersIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
    </svg>
  );
}

export default function GameAccess() {
  const currentTier = 'basic';

  const getAccessStatus = (gameTier) => {
    const gameTierIndex    = TIER_ORDER.indexOf(gameTier);
    const currentTierIndex = TIER_ORDER.indexOf(currentTier);
    return gameTierIndex <= currentTierIndex ? 'granted' : 'locked';
  };

  const getNextTier = () => {
    const idx = TIER_ORDER.indexOf(currentTier);
    return idx < TIER_ORDER.length - 1 ? TIER_ORDER[idx + 1] : null;
  };

  const nextTier = getNextTier();

  return (
    <Layout>
      <div className="game-access-page">
        {/* ── Header ── */}
        <header className="game-access-header">
          <div className="header-content">
            <span className="header-kicker">// Your Library</span>
            <h1>Game Access</h1>
            <p>All Rust-powered games available on your current subscription tier.</p>
          </div>
          <div className="header-badge">
            <span className="your-tier-label">// Current Tier</span>
            <SubscriptionBadge tier={currentTier} size="lg" showIcon />
          </div>
        </header>

        {/* ── Tier legend ── */}
        <div className="tier-legend" role="list" aria-label="Subscription tiers">
          {TIER_ORDER.map((tier) => (
            <div
              key={tier}
              role="listitem"
              className={`tier-item ${tier === currentTier ? 'current' : ''}`}
              aria-current={tier === currentTier ? 'true' : undefined}
            >
              <SubscriptionBadge tier={tier} size="sm" />
              <span className="tier-desc">{TIER_LABELS[tier]}</span>
            </div>
          ))}
        </div>

        {/* ── Games grid ── */}
        <section className="games-section" aria-label="Available games">
          <div className="games-grid">
            {MOCK_GAMES.map((game) => {
              const isLocked = getAccessStatus(game.tier) === 'locked';

              return (
                <article
                  key={game.id}
                  className={`game-access-card ${isLocked ? 'locked' : ''}`}
                  aria-label={`${game.title}${isLocked ? ' — locked' : ''}`}
                >
                  <div className="game-image-wrapper">
                    <img src={game.image} alt="" className="game-image" loading="lazy" aria-hidden="true" />
                    {isLocked && (
                      <div className="locked-overlay" aria-hidden="true">
                        <LockIcon className="lock-icon-svg" />
                        <span className="lock-tier">
                          Requires <SubscriptionBadge tier={game.tier} size="sm" />
                        </span>
                      </div>
                    )}
                  </div>

                  <div className="game-info">
                    <div className="game-header-row">
                      <h3 className="game-access-title">{game.title}</h3>
                      <SubscriptionBadge tier={game.tier} size="sm" />
                    </div>
                    <span className="game-access-category">{game.category}</span>
                    <div className="game-meta">
                      <span className="player-count-badge">
                        <PlayersIcon />
                        {game.players.toLocaleString()} players
                      </span>
                    </div>

                    {isLocked && nextTier ? (
                      <button className="btn-access-upgrade">
                        Upgrade to {nextTier.charAt(0).toUpperCase() + nextTier.slice(1)} to Unlock
                      </button>
                    ) : !isLocked ? (
                      <button className="btn-access-play">
                        ▶  Play Now
                      </button>
                    ) : null}
                  </div>
                </article>
              );
            })}
          </div>
        </section>

        {/* ── Upgrade CTA ── */}
        {nextTier && (
          <section className="upgrade-prompt" aria-label="Upgrade your subscription">
            <span className="upgrade-kicker">// Unlock More</span>
            <div className="upgrade-content">
              <h2>More Games Await</h2>
              <p>
                Upgrade to <SubscriptionBadge tier={nextTier} size="md" showIcon /> and unlock{' '}
                {MOCK_GAMES.filter(g => g.tier === nextTier).length} additional Rust-powered games —
                including <strong>{MOCK_GAMES.find(g => g.tier === nextTier)?.title}</strong> and more.
              </p>
              <button className="btn-upgrade-cta">
                Upgrade Now →
              </button>
            </div>
          </section>
        )}
      </div>
    </Layout>
  );
}

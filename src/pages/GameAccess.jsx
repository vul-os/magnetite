import Layout from '../components/Layout';
import SubscriptionBadge from '../components/SubscriptionBadge';
import './GameAccess.css';

const MOCK_GAMES = [
  {
    id: 1,
    title: 'Cosmic Drift',
    category: 'Racing',
    tier: 'free',
    players: 1247,
    image: 'https://images.unsplash.com/photo-1511882150382-421056c89033?w=400',
  },
  {
    id: 2,
    title: 'Neon Strike',
    category: 'Action',
    tier: 'basic',
    players: 892,
    image: 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?w=400',
  },
  {
    id: 3,
    title: 'Cyber Arena',
    category: 'Battle Royale',
    tier: 'pro',
    players: 3421,
    image: 'https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400',
  },
  {
    id: 4,
    title: 'Quantum Poker',
    category: 'Card Game',
    tier: 'free',
    players: 567,
    image: 'https://images.unsplash.com/photo-1526304640581-d334cdbbf45e?w=400',
  },
  {
    id: 5,
    title: 'Void Hunters',
    category: 'Adventure',
    tier: 'pro',
    players: 1823,
    image: 'https://images.unsplash.com/photo-1538481199705-c710c4e965fc?w=400',
  },
  {
    id: 6,
    title: 'Master Chess',
    category: 'Strategy',
    tier: 'unlimited',
    players: 4521,
    image: 'https://images.unsplash.com/photo-1529699211952-734e80c4d42b?w=400',
  },
];

const TIER_LABELS = {
  free: 'Free Access',
  basic: 'Basic Tier',
  pro: 'Pro Tier',
  unlimited: 'Unlimited',
};

const TIER_ORDER = ['free', 'basic', 'pro', 'unlimited'];

export default function GameAccess() {
  const currentTier = 'basic';

  const getAccessStatus = (gameTier) => {
    const gameTierIndex = TIER_ORDER.indexOf(gameTier);
    const currentTierIndex = TIER_ORDER.indexOf(currentTier);

    if (gameTierIndex <= currentTierIndex) {
      return 'granted';
    }
    return 'locked';
  };

  const getNextTier = () => {
    const currentIndex = TIER_ORDER.indexOf(currentTier);
    if (currentIndex < TIER_ORDER.length - 1) {
      return TIER_ORDER[currentIndex + 1];
    }
    return null;
  };

  const nextTier = getNextTier();

  return (
    <Layout>
      <div className="game-access-page">
        <header className="game-access-header">
          <div className="header-content">
            <h1>Game Access</h1>
            <p>View which games are available with your current subscription</p>
          </div>
          <div className="header-badge">
            <span className="your-tier-label">Your Tier</span>
            <SubscriptionBadge tier={currentTier} size="lg" showIcon />
          </div>
        </header>

        <div className="tier-legend">
          {TIER_ORDER.map((tier) => (
            <div 
              key={tier} 
              className={`tier-item ${tier === currentTier ? 'current' : ''}`}
            >
              <SubscriptionBadge tier={tier} size="sm" />
              <span className="tier-desc">{TIER_LABELS[tier]}</span>
            </div>
          ))}
        </div>

        <section className="games-section">
          <div className="games-grid">
            {MOCK_GAMES.map((game) => {
              const accessStatus = getAccessStatus(game.tier);
              const isLocked = accessStatus === 'locked';

              return (
                <div 
                  key={game.id} 
                  className={`game-access-card ${isLocked ? 'locked' : ''}`}
                >
                  <div className="game-image-wrapper">
                    <img src={game.image} alt={game.title} className="game-image" loading="lazy" />
                    {isLocked && (
                      <div className="locked-overlay">
                        <span className="lock-icon">🔒</span>
                        <span className="lock-tier">
                          Requires <SubscriptionBadge tier={game.tier} size="sm" />
                        </span>
                      </div>
                    )}
                  </div>
                  <div className="game-info">
                    <div className="game-header-row">
                      <h3 className="game-title">{game.title}</h3>
                      <SubscriptionBadge tier={game.tier} size="sm" />
                    </div>
                    <span className="game-category">{game.category}</span>
                    <div className="game-meta">
                      <span className="player-count">👥 {game.players.toLocaleString()} players</span>
                    </div>
                    {isLocked && nextTier && (
                      <button className="btn btn-primary btn-upgrade">
                        Upgrade to {nextTier.charAt(0).toUpperCase() + nextTier.slice(1)}
                      </button>
                    )}
                    {!isLocked && (
                      <button className="btn btn-secondary btn-play">
                        Play Now
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </section>

        {nextTier && (
          <section className="upgrade-prompt">
            <div className="upgrade-content">
              <h2>Unlock More Games</h2>
              <p>
                Upgrade to <SubscriptionBadge tier={nextTier} size="md" showIcon /> to access{' '}
                {MOCK_GAMES.filter(g => g.tier === nextTier).length} additional games including 
                <strong> {MOCK_GAMES.find(g => g.tier === nextTier)?.title}</strong> and more!
              </p>
              <button className="btn btn-primary btn-lg">
                Upgrade Now
              </button>
            </div>
          </section>
        )}
      </div>
    </Layout>
  );
}

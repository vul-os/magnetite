import { useState } from 'react';
import Layout from '../components/Layout';
import GameCard from '../components/GameCard';
import GameGallery from '../components/GameGallery';
import ReviewList from '../components/ReviewList';
import './GameDetail.css';

const REQUIRED_TIER = {
  free: ['free'],
  basic: ['free', 'basic'],
  pro: ['free', 'basic', 'pro'],
  unlimited: ['free', 'basic', 'pro', 'unlimited'],
};

const TIER_NAMES = {
  free: 'Free',
  basic: 'Basic',
  pro: 'Pro',
  unlimited: 'Unlimited',
};

const MOCK_GAME = {
  id: 1,
  title: 'Cosmic Raiders',
  developer: 'StarForge Studios',
  developerId: 'starforge',
  requiredTier: 'basic',
  isFree: false,
  rating: 4.7,
  category: 'Action',
  description:
    'Embark on an interstellar adventure in Cosmic Raiders — a server-authoritative Rust game compiled to WebAssembly. Battle alien forces across 12 unique star systems, upgrade your spacecraft, and compete against players worldwide. The entire game logic is authored in Rust with Bevy, running with deterministic netcode on the Magnetite platform.',
  thumbnail: 'https://picsum.photos/seed/game1/1920/600',
  screenshots: [
    'https://picsum.photos/seed/ss1/1920/1080',
    'https://picsum.photos/seed/ss2/1920/1080',
    'https://picsum.photos/seed/ss3/1920/1080',
    'https://picsum.photos/seed/ss4/1920/1080',
  ],
  video: 'https://www.youtube.com/embed/dQw4w9WgXcQ',
  github: 'https://github.com/starforge/cosmic-raiders',
  release_date: '2026-03-15',
  players_min: 1,
  players_max: 4,
  system_requirements: {
    os: 'Any modern browser (WASM) / Windows 10 / macOS Ventura+',
    processor: 'Intel Core i5-8400 / AMD Ryzen 5 2600',
    memory: '8 GB RAM',
    graphics: 'NVIDIA GTX 1060 / AMD RX 580 (native) or WebGPU',
    storage: '15 GB (native) / streamed in-browser',
  },
  achievements: [
    { name: 'First Victory', description: 'Win your first match', progress: 1, total: 1 },
    { name: 'Space Explorer', description: 'Visit all 12 star systems', progress: 9, total: 12 },
    { name: 'Ace Pilot', description: 'Achieve 100 kills in a single match', progress: 45, total: 100 },
    { name: 'Team Player', description: 'Complete 50 co-op missions', progress: 15, total: 50 },
  ],
};

const MOCK_LEADERBOARD = [
  { rank: 1, player: 'NebulaKing',  score: 15420, avatar: 'https://picsum.photos/seed/p1/40/40' },
  { rank: 2, player: 'SpaceAce',    score: 14850, avatar: 'https://picsum.photos/seed/p2/40/40' },
  { rank: 3, player: 'AstroNinja',  score: 13200, avatar: 'https://picsum.photos/seed/p3/40/40' },
  { rank: 4, player: 'VoidWalker',  score: 11900, avatar: 'https://picsum.photos/seed/p4/40/40' },
  { rank: 5, player: 'StarDust99',  score: 10500, avatar: 'https://picsum.photos/seed/p5/40/40' },
  { rank: 6, player: 'CosmicWind',  score:  9800, avatar: 'https://picsum.photos/seed/p6/40/40' },
  { rank: 7, player: 'GalaxyPro',   score:  9200, avatar: 'https://picsum.photos/seed/p7/40/40' },
  { rank: 8, player: 'NovaFlare',   score:  8700, avatar: 'https://picsum.photos/seed/p8/40/40' },
  { rank: 9, player: 'PulsarX',     score:  8100, avatar: 'https://picsum.photos/seed/p9/40/40' },
  { rank: 10, player: 'QuasarKing', score:  7500, avatar: 'https://picsum.photos/seed/p10/40/40' },
];

const MOCK_REVIEWS = [
  { user: 'GameMaster42',  rating: 5, comment: 'Best space shooter I have ever played! The Rust WASM build is silky smooth.', date: '2026-05-10', helpful: 24 },
  { user: 'PocketRocket',  rating: 4, comment: 'Great visuals and gameplay. Matchmaking could be slightly faster — otherwise fantastic!', date: '2026-05-08', helpful: 18 },
  { user: 'CosmicDrifter', rating: 5, comment: 'Addictive gameplay loop. The server-authoritative Rust netcode means zero desync.', date: '2026-05-05', helpful: 31 },
  { user: 'SpaceExplorer', rating: 4, comment: 'Really enjoy the co-op missions. Would love more boss fights in future updates.', date: '2026-05-02', helpful: 12 },
];

const MOCK_SIMILAR_GAMES = [
  { id: 2, title: 'Nebula Strike',    developer: 'Void Studios', requiredTier: 'pro',   isFree: false, rating: 4.5, thumbnail: 'https://picsum.photos/seed/game2/400/225', players_online: 342 },
  { id: 3, title: 'Stellar Conquest', developer: 'Orbit Games',  requiredTier: 'basic', isFree: false, rating: 4.2, thumbnail: 'https://picsum.photos/seed/game3/400/225', players_online: 128 },
  { id: 4, title: 'Galaxy Warfare',   developer: 'Astro Inc',    requiredTier: 'free',  isFree: true,  rating: 4.8, thumbnail: 'https://picsum.photos/seed/game4/400/225', players_online: 567 },
];

const TABS = ['Overview', 'Leaderboard', 'Reviews'];

function StarRating({ rating, size = 'md' }) {
  const fullStars  = Math.floor(rating);
  const hasHalf    = rating % 1 >= 0.5;
  const emptyStars = 5 - fullStars - (hasHalf ? 1 : 0);
  const px         = size === 'sm' ? 14 : size === 'lg' ? 22 : 16;

  return (
    <div className="star-rating" aria-label={`${rating} out of 5 stars`}>
      {[...Array(fullStars)].map((_, i) => (
        <svg key={`full-${i}`} className="star full" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      {hasHalf && (
        <svg className="star half" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <defs>
            <linearGradient id="halfGrad">
              <stop offset="50%" stopColor="currentColor" />
              <stop offset="50%" stopColor="var(--color-border-strong)" />
            </linearGradient>
          </defs>
          <path fill="url(#halfGrad)" d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      )}
      {[...Array(emptyStars)].map((_, i) => (
        <svg key={`empty-${i}`} className="star empty" viewBox="0 0 24 24" fill="currentColor" width={px} height={px} aria-hidden="true">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      <span className="rating-value" style={{ fontSize: px - 2 }}>{rating.toFixed(1)}</span>
    </div>
  );
}

function AchievementCard({ achievement }) {
  const pct = Math.round((achievement.progress / achievement.total) * 100);
  return (
    <div className="achievement-card">
      <div className="achievement-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      </div>
      <div className="achievement-info">
        <h4>{achievement.name}</h4>
        <p>{achievement.description}</p>
        <div className="achievement-progress">
          <div className="progress-bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <span className="progress-text">{achievement.progress}/{achievement.total}</span>
        </div>
      </div>
    </div>
  );
}

export default function GameDetail() {
  const [walletConnected, setWalletConnected]   = useState(false);
  const [inQueue, setInQueue]                   = useState(false);
  const [activeTab, setActiveTab]               = useState('Overview');
  const [wishlisted, setWishlisted]             = useState(false);
  const [showShareToast, setShowShareToast]     = useState(false);
  const [userTier] = useState('basic');

  const userTierLevel     = Object.keys(REQUIRED_TIER).indexOf(userTier);
  const requiredTierLevel = Object.keys(REQUIRED_TIER).indexOf(MOCK_GAME.requiredTier);
  const hasAccess         = userTierLevel >= requiredTierLevel || MOCK_GAME.isFree;
  const needsUpgrade      = !hasAccess && !MOCK_GAME.isFree;

  const handlePlayNow = () => {
    if (!walletConnected) { setWalletConnected(true); return; }
    if (needsUpgrade)     { window.location.href = '/pricing'; return; }
    setInQueue(true);
    setTimeout(() => setInQueue(false), 3000);
  };

  const handleShare = () => {
    navigator.clipboard.writeText(window.location.href);
    setShowShareToast(true);
    setTimeout(() => setShowShareToast(false), 2000);
  };

  const formatDate = (d) =>
    new Date(d).toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });

  const renderTabContent = () => {
    switch (activeTab) {
      case 'Overview':
        return (
          <div className="tab-overview">
            <GameGallery images={MOCK_GAME.screenshots} title={MOCK_GAME.title} />

            <div className="overview-section">
              <h3>// About This Game</h3>
              <p className="game-description">{MOCK_GAME.description}</p>
            </div>

            {MOCK_GAME.video && (
              <div className="video-section">
                <h3>// Gameplay Video</h3>
                <div className="video-container">
                  <iframe
                    src={MOCK_GAME.video}
                    title="Gameplay Video"
                    frameBorder="0"
                    allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                    allowFullScreen
                  />
                </div>
              </div>
            )}

            <div className="achievements-section">
              <div className="section-header-row">
                <h3 style={{ margin: 0 }}>// Achievements</h3>
                <span className="achievement-count">{MOCK_GAME.achievements.length} Total</span>
              </div>
              <div className="achievements-grid">
                {MOCK_GAME.achievements.map((a, i) => (
                  <AchievementCard key={i} achievement={a} />
                ))}
              </div>
            </div>
          </div>
        );

      case 'Leaderboard':
        return (
          <div className="tab-leaderboard">
            <div className="leaderboard-header">
              <h3>// Top Players</h3>
              <a href="/leaderboard" className="view-all-link">View Full Leaderboard →</a>
            </div>
            <div className="leaderboard-list">
              {MOCK_LEADERBOARD.map(entry => (
                <div key={entry.rank} className={`leaderboard-row rank-${entry.rank}`}>
                  <span className="rank">#{entry.rank}</span>
                  <img src={entry.avatar} alt="" className="player-avatar" loading="lazy" aria-hidden="true" />
                  <span className="player-name">{entry.player}</span>
                  <span className="player-score">{entry.score.toLocaleString()}</span>
                </div>
              ))}
            </div>
          </div>
        );

      case 'Reviews':
        return (
          <div className="tab-reviews">
            <div className="reviews-summary">
              <div className="rating-overview">
                <div className="big-rating">{MOCK_GAME.rating}</div>
                <StarRating rating={MOCK_GAME.rating} size="lg" />
                <p>{MOCK_REVIEWS.length} Reviews</p>
              </div>
              <div className="rating-breakdown">
                {[5, 4, 3, 2, 1].map(stars => {
                  const count   = MOCK_REVIEWS.filter(r => r.rating === stars).length;
                  const percent = (count / MOCK_REVIEWS.length) * 100;
                  return (
                    <div key={stars} className="rating-row">
                      <span>{stars} Stars</span>
                      <div className="rating-bar">
                        <div className="rating-fill" style={{ width: `${percent}%` }} />
                      </div>
                      <span className="rating-count">{count}</span>
                    </div>
                  );
                })}
              </div>
            </div>
            <ReviewList
              reviews={MOCK_REVIEWS.map((r, i) => ({ ...r, id: i }))}
              walletConnected={walletConnected}
              onCreateReview={() => {}}
              onHelpful={(id) => console.log('Helpful:', id)}
              onReport={(id) => console.log('Report:', id)}
            />
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <Layout>
      <div className="game-detail">
        {/* ── Hero ── */}
        <section className="game-hero" aria-label={`${MOCK_GAME.title} hero`}>
          <div className="hero-image-container">
            <img src={MOCK_GAME.thumbnail} alt={MOCK_GAME.title} className="hero-image" loading="eager" />
            <div className="hero-overlay" aria-hidden="true" />
            <div className="hero-gradient" aria-hidden="true" />
          </div>
          <div className="hero-content">
            <div className="hero-top">
              <span className="category-badge">{MOCK_GAME.category}</span>
              {MOCK_GAME.isFree ? (
                <span className="tier-badge free">Free to Play</span>
              ) : (
                <span className="tier-badge">
                  {TIER_NAMES[MOCK_GAME.requiredTier]} Tier
                </span>
              )}
            </div>
            <div className="hero-main">
              <h1 className="game-title">{MOCK_GAME.title}</h1>
              <p className="game-developer">
                by{' '}
                <a href={`/developer/${MOCK_GAME.developerId}`}>
                  {MOCK_GAME.developer}
                </a>
              </p>
              <StarRating rating={MOCK_GAME.rating} size="lg" />
            </div>
            <div className="hero-actions">
              <div className="action-buttons">
                <button
                  className="btn-play"
                  onClick={handlePlayNow}
                  disabled={inQueue}
                  aria-label={
                    !walletConnected ? 'Connect wallet and play' :
                    inQueue ? 'Joining game queue' :
                    needsUpgrade ? `Upgrade to ${TIER_NAMES[MOCK_GAME.requiredTier]} to play` :
                    'Play Now'
                  }
                >
                  {!walletConnected
                    ? 'Connect & Play'
                    : inQueue
                    ? 'Joining...'
                    : needsUpgrade
                    ? 'Upgrade to Play'
                    : '▶  Play Now'}
                </button>
                <button
                  className={`btn-wishlist ${wishlisted ? 'active' : ''}`}
                  onClick={() => setWishlisted(w => !w)}
                  aria-label={wishlisted ? 'Remove from wishlist' : 'Add to wishlist'}
                  aria-pressed={wishlisted}
                >
                  <svg viewBox="0 0 24 24" fill={wishlisted ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
                  </svg>
                </button>
              </div>
              <div className="price-display">
                {MOCK_GAME.isFree ? (
                  <span className="price-free">// Free to Play</span>
                ) : hasAccess ? (
                  <span className="price-included">// Included in your plan</span>
                ) : (
                  <span className="price-upgrade">// Requires {TIER_NAMES[MOCK_GAME.requiredTier]}</span>
                )}
              </div>
            </div>
            {needsUpgrade && (
              <div className="upgrade-prompt" role="alert">
                <p>Upgrade your subscription to unlock this game</p>
                <a href="/pricing" className="btn btn-primary">View Plans</a>
              </div>
            )}
          </div>
        </section>

        {/* ── Info bar ── */}
        <nav className="info-bar" aria-label="Game metadata">
          <div className="info-bar-inner">
            <div className="info-item">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
                <circle cx="9" cy="7" r="4" />
                <path d="M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
              </svg>
              <span className="info-label">Players</span>
              <span className="info-value">{MOCK_GAME.players_min}–{MOCK_GAME.players_max}</span>
            </div>
            <div className="info-divider" aria-hidden="true" />
            <div className="info-item">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
                <line x1="16" y1="2" x2="16" y2="6" />
                <line x1="8" y1="2" x2="8" y2="6" />
                <line x1="3" y1="10" x2="21" y2="10" />
              </svg>
              <span className="info-label">Released</span>
              <span className="info-value">{formatDate(MOCK_GAME.release_date)}</span>
            </div>
            <div className="info-divider" aria-hidden="true" />
            <div className="info-item">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
              </svg>
              <a href={MOCK_GAME.github} target="_blank" rel="noopener noreferrer" className="info-link">
                Source on GitHub ↗
              </a>
            </div>
            <div className="info-divider" aria-hidden="true" />
            <div className="info-item">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
              </svg>
              <span className="info-label">Runtime</span>
              <span className="info-value">Rust + Bevy → WASM</span>
            </div>
          </div>
        </nav>

        {/* ── Main grid ── */}
        <div className="game-content">
          <main className="game-main">
            <div className="tabs-section">
              <div className="tabs-nav" role="tablist">
                {TABS.map(tab => (
                  <button
                    key={tab}
                    role="tab"
                    aria-selected={activeTab === tab}
                    className={`tab-btn ${activeTab === tab ? 'active' : ''}`}
                    onClick={() => setActiveTab(tab)}
                  >
                    {tab}
                  </button>
                ))}
              </div>
              <div className="tab-content" role="tabpanel">
                {renderTabContent()}
              </div>
            </div>
          </main>

          <aside className="game-sidebar" aria-label="Game details">
            <div className="sidebar-card developer-card">
              <h3>// Developer</h3>
              <div className="developer-info">
                <img
                  src={`https://picsum.photos/seed/${MOCK_GAME.developerId}/80/80`}
                  alt={MOCK_GAME.developer}
                  className="developer-avatar"
                  loading="lazy"
                />
                <div className="developer-details">
                  <span className="developer-name">{MOCK_GAME.developer}</span>
                  <a href={`/developer/${MOCK_GAME.developerId}`} className="developer-link">
                    View Profile →
                  </a>
                </div>
              </div>
            </div>

            <div className="sidebar-card system-req-card">
              <h3>// System Requirements</h3>
              <ul className="system-req-list">
                {Object.entries(MOCK_GAME.system_requirements).map(([key, value]) => (
                  <li key={key}>
                    <span className="req-label">{key}</span>
                    {value}
                  </li>
                ))}
              </ul>
            </div>

            <div className="sidebar-card similar-games-card">
              <h3>// Similar Games</h3>
              <div className="similar-games-list">
                {MOCK_SIMILAR_GAMES.map(game => (
                  <GameCard key={game.id} game={game} showPlayButton={false} />
                ))}
              </div>
            </div>

            <div className="sidebar-card actions-card">
              <div className="action-links">
                <button className="action-link-btn" onClick={handleShare}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <circle cx="18" cy="5" r="3" />
                    <circle cx="6" cy="12" r="3" />
                    <circle cx="18" cy="19" r="3" />
                    <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
                    <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
                  </svg>
                  Share Game
                </button>
                <button className="action-link-btn danger">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                    <circle cx="12" cy="12" r="10" />
                    <line x1="12" y1="8" x2="12" y2="12" />
                    <line x1="12" y1="16" x2="12.01" y2="16" />
                  </svg>
                  Report Issue
                </button>
              </div>
            </div>
          </aside>
        </div>

        {showShareToast && (
          <div className="toast-notification" role="status" aria-live="polite">
            // Link copied to clipboard
          </div>
        )}
      </div>
    </Layout>
  );
}

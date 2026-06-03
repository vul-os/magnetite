import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import SubscriptionBadge from '../components/SubscriptionBadge';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './GameAccess.css';

// Fallback shown when the API is unavailable (VITE_USE_MOCKS=true or backend down)
const PLACEHOLDER_GAMES = [
  { id: 1, title: 'Cosmic Drift',  category: 'Racing',        tier: 'free',      players: 1247, image: 'https://images.unsplash.com/photo-1511882150382-421056c89033?w=400' },
  { id: 2, title: 'Neon Strike',   category: 'Action',        tier: 'basic',     players: 892,  image: 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?w=400' },
  { id: 3, title: 'Cyber Arena',   category: 'Battle Royale', tier: 'pro',       players: 3421, image: 'https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400' },
  { id: 4, title: 'Quantum Poker', category: 'Card Game',     tier: 'free',      players: 567,  image: 'https://images.unsplash.com/photo-1526304640581-d334cdbbf45e?w=400' },
  { id: 5, title: 'Void Hunters',  category: 'Adventure',     tier: 'pro',       players: 1823, image: 'https://images.unsplash.com/photo-1538481199705-c710c4e965fc?w=400' },
  { id: 6, title: 'Master Chess',  category: 'Strategy',      tier: 'unlimited', players: 4521, image: 'https://images.unsplash.com/photo-1529699211952-734e80c4d42b?w=400' },
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
  const { t } = useTranslation();
  const navigate = useNavigate();

  const [games, setGames]           = useState(null);   // null = loading
  const [currentTier, setCurrentTier] = useState(null); // null = loading
  const [loadError, setLoadError]   = useState(null);

  const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

  // Load games/tier from the API (external system); the mock branch seeds the
  // same state synchronously.
  useEffect(() => {
    if (useMocks) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setGames(PLACEHOLDER_GAMES);
      setCurrentTier('free');
      return;
    }

    let cancelled = false;

    async function load() {
      try {
        const [gamesData, subData] = await Promise.allSettled([
          api.games.list(),
          api.subscriptions.current(),
        ]);

        if (cancelled) return;

        // Games
        if (gamesData.status === 'fulfilled' && gamesData.value) {
          const list = Array.isArray(gamesData.value)
            ? gamesData.value
            : (gamesData.value?.games ?? null);
          setGames(list && list.length > 0 ? list : PLACEHOLDER_GAMES);
        } else {
          setGames(PLACEHOLDER_GAMES);
        }

        // Subscription tier
        if (subData.status === 'fulfilled' && subData.value) {
          const tier = subData.value?.plan ?? subData.value?.tier ?? subData.value?.plan_id ?? 'free';
          setCurrentTier(tier);
        } else {
          setCurrentTier('free');
        }
      } catch (err) {
        if (!cancelled) {
          setLoadError(err.message);
          setGames(PLACEHOLDER_GAMES);
          setCurrentTier('free');
        }
      }
    }

    load();
    return () => { cancelled = true; };
  }, [useMocks]);

  const loading = games === null || currentTier === null;

  const getAccessStatus = (gameTier) => {
    if (!currentTier) return 'locked';
    const gameTierIndex    = TIER_ORDER.indexOf(gameTier ?? 'free');
    const currentTierIndex = TIER_ORDER.indexOf(currentTier);
    return gameTierIndex <= currentTierIndex ? 'granted' : 'locked';
  };

  const nextTier = currentTier
    ? (TIER_ORDER.indexOf(currentTier) < TIER_ORDER.length - 1
        ? TIER_ORDER[TIER_ORDER.indexOf(currentTier) + 1]
        : null)
    : null;

  const displayGames = games ?? [];

  return (
    <Layout>
      <div className="game-access-page">
        {/* ── Header ── */}
        <header className="game-access-header" aria-labelledby="game-access-heading">
          <div className="header-content">
            <span className="header-kicker">{t('game.accessKicker')}</span>
            <h1 id="game-access-heading">{t('game.accessTitle')}</h1>
            <p>{t('game.accessSubtitle')}</p>
          </div>
          <div className="header-badge">
            <span className="your-tier-label">{t('game.currentTierKicker')}</span>
            {loading
              ? <span className="tier-loading" aria-busy="true">{t('game.tierLoading')}</span>
              : <SubscriptionBadge tier={currentTier} size="lg" showIcon />
            }
          </div>
        </header>

        {/* ── Error banner ── */}
        {loadError && (
          <div className="game-access-error" role="alert">
            {t('game.subscriptionError')}
          </div>
        )}

        {/* ── Tier legend ── */}
        <div className="tier-legend" role="list" aria-label={t('game.tiersLabel')}>
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
        <section className="games-section" aria-label={t('game.gamesLabel')}>
          {loading ? (
            <div className="loading-state" aria-live="polite" aria-busy="true">
              <span className="spinner" aria-hidden="true" />
              <span>{t('game.loadingGames')}</span>
            </div>
          ) : (
            <div className="games-grid">
              {displayGames.map((game) => {
                const isLocked = getAccessStatus(game.tier) === 'locked';
                const imgSrc   = game.image ?? game.thumbnail ?? game.cover_image;

                return (
                  <article
                    key={game.id}
                    className={`game-access-card ${isLocked ? 'locked' : ''}`}
                    aria-label={isLocked ? t('game.gameLockedLabel', { title: game.title }) : game.title}
                  >
                    <div className="game-image-wrapper">
                      {imgSrc && (
                        <img src={imgSrc} alt={game.title} className="game-image" loading="lazy" />
                      )}
                      {isLocked && (
                        <div className="locked-overlay" aria-hidden="true">
                          <LockIcon className="lock-icon-svg" />
                          <span className="lock-tier">
                            {t('game.requiresTier')} <SubscriptionBadge tier={game.tier ?? 'pro'} size="sm" />
                          </span>
                        </div>
                      )}
                    </div>

                    <div className="game-info">
                      <div className="game-header-row">
                        <h3 className="game-access-title">{game.title}</h3>
                        <SubscriptionBadge tier={game.tier ?? 'free'} size="sm" />
                      </div>
                      <span className="game-access-category">{game.category}</span>
                      {game.players != null && (
                        <div className="game-meta">
                          <span className="player-count-badge">
                            <PlayersIcon />
                            {t('game.playersCount', { count: Number(game.players).toLocaleString() })}
                          </span>
                        </div>
                      )}

                      {isLocked && nextTier ? (
                        <button
                          className="btn-access-upgrade"
                          onClick={() => navigate('/subscription')}
                          aria-label={t('game.upgradeTo', { tier: nextTier.charAt(0).toUpperCase() + nextTier.slice(1) })}
                        >
                          {t('game.upgradeTo', { tier: nextTier.charAt(0).toUpperCase() + nextTier.slice(1) })}
                        </button>
                      ) : !isLocked ? (
                        <button
                          className="btn-access-play"
                          onClick={() => navigate(`/matchmaking?game=${game.id}`)}
                          aria-label={`${t('game.playNow')} ${game.title}`}
                        >
                          {t('game.playNow')}
                        </button>
                      ) : null}
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </section>

        {/* ── Upgrade CTA ── */}
        {!loading && nextTier && (
          <section className="upgrade-prompt" aria-labelledby="upgrade-heading">
            <span className="upgrade-kicker">{t('game.moreGamesKicker')}</span>
            <div className="upgrade-content">
              <h2 id="upgrade-heading">{t('game.moreGamesTitle')}</h2>
              <p>
                Upgrade to <SubscriptionBadge tier={nextTier} size="md" showIcon /> and unlock additional
                Rust-powered games.
              </p>
              <button className="btn-upgrade-cta" onClick={() => navigate('/subscription')}>
                {t('game.upgradeNow')}
              </button>
            </div>
          </section>
        )}
      </div>
    </Layout>
  );
}

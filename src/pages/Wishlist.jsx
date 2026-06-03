import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import GameCard from '../components/GameCard';
import Button from '../components/common/Button';
import EmptyState from '../components/empty/EmptyState';
import { mockGames } from '../data/mockGames';
import { api } from '../api/client';
import { useTranslation } from '../i18n/useTranslation';
import './social.css';

const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

const WishlistEmptyIcon = (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
  </svg>
);

export default function Wishlist() {
  const { t } = useTranslation();
  const [wishlistGames, setWishlistGames] = useState(useMocks ? mockGames.slice(0, 3) : []);
  const [removingId, setRemovingId]       = useState(null);
  const [loading, setLoading]             = useState(true);

  useEffect(() => {
    if (useMocks) {
      setLoading(false);
      return;
    }
    let cancelled = false;

    async function loadWishlist() {
      try {
        const data = await api.wishlist.list();
        if (!cancelled && data) {
          const list = Array.isArray(data) ? data : (data?.games ?? data?.items ?? null);
          if (list != null) setWishlistGames(list);
        }
      } catch { /* API unavailable — empty list is the honest state */ } finally {
        if (!cancelled) setLoading(false);
      }
    }

    loadWishlist();
    return () => { cancelled = true; };
  }, []);

  const handleRemove = async (gameId) => {
    setRemovingId(gameId);
    try {
      await api.wishlist.remove(gameId);
    } catch { /* optimistic remove */ }
    setWishlistGames(prev => prev.filter(game => game.id !== gameId));
    setRemovingId(null);
  };

  const gameCount = wishlistGames.length;

  return (
    <Layout>
      <div className="wishlist-page reveal">
        <header className="page-header reveal-1">
          <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', flexWrap: 'wrap' }}>
            <div>
              <span className="kicker">// {t('wishlist.kicker')}</span>
              <h1>{t('wishlist.title')}</h1>
            </div>
            {!loading && (
              <span className="wishlist-count" aria-live="polite" aria-label={t('wishlist.countLabel', { count: gameCount })}>
                {gameCount} {gameCount === 1 ? t('wishlist.game') : t('wishlist.games')}
              </span>
            )}
          </div>
        </header>

        {loading ? (
          <div className="loading-state reveal-2" aria-live="polite" aria-busy="true">
            <span className="spinner" aria-hidden="true" />
            <span>{t('wishlist.loading')}</span>
          </div>
        ) : wishlistGames.length === 0 ? (
          <div className="reveal-2">
            <EmptyState
              icon={WishlistEmptyIcon}
              title={t('wishlist.emptyTitle')}
              description={t('wishlist.emptyDesc')}
              action={
                <Button onClick={() => window.location.href = '/marketplace'}>
                  {t('wishlist.browseMarketplace')}
                </Button>
              }
            />
          </div>
        ) : (
          <div className="wishlist-grid reveal-2">
            {wishlistGames.map(game => (
              <div key={game.id} className="wishlist-item">
                <GameCard game={game} showPlayButton={false} />
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => handleRemove(game.id)}
                  loading={removingId === game.id}
                  className="remove-button"
                  aria-label={t('wishlist.removeLabel', { title: game.title })}
                >
                  {t('wishlist.remove')}
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </Layout>
  );
}

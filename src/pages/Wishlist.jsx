import { useState, useEffect } from 'react';
import Layout from '../components/Layout';
import GameCard from '../components/GameCard';
import Button from '../components/common/Button';
import { mockGames } from '../data/mockGames';
import { api } from '../api/client';
import './social.css';

export default function Wishlist() {
  const [wishlistGames, setWishlistGames] = useState(mockGames.slice(0, 3));
  const [removingId, setRemovingId] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadWishlist() {
      try {
        const data = await api.wishlist.list();
        if (!cancelled && data) {
          const list = Array.isArray(data) ? data : (data?.games ?? data?.items ?? null);
          if (list) setWishlistGames(list);
        }
      } catch { /* use mock data */ } finally {
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

  return (
    <Layout>
      <div className="wishlist-page">
        <header className="page-header">
          <h1>My Wishlist</h1>
          <span className="wishlist-count">
            {wishlistGames.length} {wishlistGames.length === 1 ? 'game' : 'games'}
          </span>
        </header>

        {loading ? (
          <div className="loading-state">
            <span className="spinner" />
            <span>Loading wishlist…</span>
          </div>
        ) : wishlistGames.length === 0 ? (
          <div className="empty-state" style={{ padding: '4rem 1.5rem' }}>
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              className="empty-icon"
              aria-hidden="true"
            >
              <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
            </svg>
            <h2 className="empty-title">Your wishlist is empty</h2>
            <p className="empty-description">Save Rust games to your wishlist to revisit them later</p>
            <div className="empty-action">
              <Button onClick={() => window.location.href = '/marketplace'}>
                Browse Marketplace
              </Button>
            </div>
          </div>
        ) : (
          <div className="wishlist-grid">
            {wishlistGames.map(game => (
              <div key={game.id} className="wishlist-item">
                <GameCard game={game} showPlayButton={false} />
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => handleRemove(game.id)}
                  loading={removingId === game.id}
                  className="remove-button"
                >
                  Remove from Wishlist
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </Layout>
  );
}

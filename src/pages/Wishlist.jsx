import { useState } from 'react';
import Layout from '../components/Layout';
import GameCard from '../components/GameCard';
import Button from '../components/common/Button';
import { mockGames } from '../data/mockGames';

export default function Wishlist() {
  const [wishlistGames, setWishlistGames] = useState(mockGames.slice(0, 3));
  const [loading, setLoading] = useState(false);

  const handleRemove = async (gameId) => {
    setLoading(true);
    await new Promise(resolve => setTimeout(resolve, 300));
    setWishlistGames(wishlistGames.filter(game => game.id !== gameId));
    setLoading(false);
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

        {wishlistGames.length === 0 ? (
          <div className="empty-state">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              className="empty-icon"
            >
              <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
            </svg>
            <h2>Your wishlist is empty</h2>
            <p>Start adding games to your wishlist to save them for later</p>
            <Button onClick={() => window.location.href = '/marketplace'}>
              Browse Marketplace
            </Button>
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
                  loading={loading}
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
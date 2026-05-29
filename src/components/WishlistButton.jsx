import { useState } from 'react';
import Spinner from './common/Spinner';

export default function WishlistButton({ gameId, initialIsWishlisted = false, onToggle }) {
  const [isWishlisted, setIsWishlisted] = useState(initialIsWishlisted);
  const [loading, setLoading] = useState(false);

  const handleClick = async (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    setLoading(true);
    
    try {
      if (onToggle) {
        await onToggle(gameId, !isWishlisted);
      }
      setIsWishlisted(!isWishlisted);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      className={`wishlist-button ${isWishlisted ? 'wishlisted' : ''}`}
      onClick={handleClick}
      disabled={loading}
      aria-label={isWishlisted ? 'Remove from wishlist' : 'Add to wishlist'}
    >
      {loading ? (
        <Spinner size="sm" />
      ) : (
        <svg
          viewBox="0 0 24 24"
          fill={isWishlisted ? 'currentColor' : 'none'}
          stroke="currentColor"
          strokeWidth="2"
          className="heart-icon"
        >
          <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z" />
        </svg>
      )}
    </button>
  );
}
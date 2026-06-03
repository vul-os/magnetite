import { useState, memo } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from '../i18n/useTranslation';
import './GameCard.css';

const StarRating = memo(function StarRating({ rating }) {
  const fullStars = Math.floor(rating);
  const hasHalfStar = rating % 1 >= 0.5;
  const emptyStars = 5 - fullStars - (hasHalfStar ? 1 : 0);

  return (
    <div className="star-rating">
      {[...Array(fullStars)].map((_, i) => (
        <svg key={`full-${i}`} className="star full" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      {hasHalfStar && (
        <svg className="star half" viewBox="0 0 24 24" fill="currentColor">
          <defs>
            <linearGradient id="halfGradient">
              <stop offset="50%" stopColor="currentColor" />
              <stop offset="50%" stopColor="rgba(255,255,255,0.15)" />
            </linearGradient>
          </defs>
          <path fill="url(#halfGradient)" d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      )}
      {[...Array(emptyStars)].map((_, i) => (
        <svg key={`empty-${i}`} className="star empty" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      ))}
      <span className="rating-value">{rating.toFixed(1)}</span>
    </div>
  );
});

const Badge = memo(function Badge({ type }) {
  if (!type) return null;
  return <span className={`card-badge ${type.toLowerCase()}`}>{type}</span>;
});

const GameCardSkeleton = memo(function GameCardSkeleton() {
  return (
    <div className="game-card skeleton">
      <div className="card-image" />
      <div className="card-content">
        <div className="skeleton-title" />
        <div className="skeleton-developer" />
        <div className="skeleton-footer">
          <div className="skeleton-price" />
          <div className="skeleton-rating" />
        </div>
      </div>
    </div>
  );
});

const ErrorState = memo(function ErrorState() {
  return (
    <div className="card-image error">
      <svg className="error-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <circle cx="8.5" cy="8.5" r="1.5" />
        <path d="M21 15l-5-5L5 21" />
      </svg>
    </div>
  );
});

export default memo(function GameCard({ game, loading = false, showPlayButton = true }) {
  const { t } = useTranslation();
  const [imageError, setImageError] = useState(false);

  if (loading || !game) {
    return <GameCardSkeleton />;
  }

  const playersOnline = game.players_online ?? 0;
  const isPopular = playersOnline > 100;
  const isNew = game.is_new || false;

  const thumbnailSrc = imageError
    ? null
    : (game.thumbnail || `https://picsum.photos/seed/${game.id}/400/225`);

  return (
    <article className="game-card" aria-label={game.title}>
      <Link to={`/game/${game.id}`} className="card-image-link" tabIndex={-1} aria-hidden="true">
        <div className="card-image">
          {thumbnailSrc ? (
            <img
              src={thumbnailSrc}
              alt=""
              loading="lazy"
              onError={() => setImageError(true)}
            />
          ) : (
            <ErrorState />
          )}
          {isPopular && !imageError && <Badge type="Popular" />}
          {isNew && !imageError && <Badge type="New" />}
          {showPlayButton && !imageError && (
            <div className="play-overlay" aria-hidden="true">
              <span className="play-button">{t('games.play')}</span>
            </div>
          )}
        </div>
      </Link>

      <div className="card-content">
        <Link to={`/game/${game.id}`} className="card-title-link">
          <h3 className="card-title">{game.title}</h3>
        </Link>
        <p className="developer">{game.developer}</p>

        <div className="card-meta">
          <StarRating rating={game.rating ?? 0} />
          {playersOnline > 0 && (
            <span className="players-online" role="img" aria-label={`${playersOnline.toLocaleString()} ${t('games.players')} online`}>
              <svg className="online-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <circle cx="12" cy="12" r="4" />
              </svg>
              <span aria-hidden="true">{playersOnline.toLocaleString()} online</span>
            </span>
          )}
        </div>

        <div className="card-footer">
          <span className="price" role="img" aria-label={`${game.fee_per_session} USD per session`}>
            <span className="price-value" aria-hidden="true">{game.fee_per_session}</span>
            <span className="price-currency" aria-hidden="true">USDC</span>
            <span className="price-period" aria-hidden="true">/{t('games.sessions')}</span>
          </span>
        </div>
      </div>
    </article>
  );
});

import { useState, memo } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from '../i18n/useTranslation';
import './GameCard.css';

const StarRating = memo(function StarRating({ rating }) {
  // Honest absence, not an invented 0.0 — a game with no rating yet is
  // unrated, not "rated zero". See DESIGN.md §7 (never invent a placeholder
  // rating) and GameDetail's identical rule for the same widget.
  if (typeof rating !== 'number' || Number.isNaN(rating) || rating <= 0) return null;
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

// A small, stable hash so the generated tile is deterministic per game — the
// same title always draws the same field signature.
function hashString(str) {
  let h = 2166136261;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

/*
 * BrandTile — the honest fallback when a game ships no artwork. NOT a stock
 * photo and never presented as a screenshot: it is a generated "field
 * signature" — a dipole of magnetic field-lines whose curvature and hue are
 * derived from the game's own identity, plus its initials. It reads clearly as
 * platform-generated art, which is exactly the honest thing to show for a game
 * with no thumbnail (see DESIGN.md §7 / GameCard's no-stock-photo rule).
 */
const BrandTile = memo(function BrandTile({ seedText }) {
  const h = hashString(seedText || 'magnetite');
  const rot = h % 40 - 20;                 // field tilt
  const spread = 60 + (h >> 5) % 80;       // arc spread
  const cy = 90 + (h >> 9) % 40;           // core height
  const initials = (seedText || '?')
    .split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('').toUpperCase();

  return (
    <div className="card-image brand-tile" aria-hidden="true">
      <svg className="brand-tile-field" viewBox="0 0 320 180" preserveAspectRatio="xMidYMid slice" fill="none">
        <g stroke="currentColor" strokeLinecap="round" style={{ transform: `rotate(${rot}deg)`, transformOrigin: '160px 90px' }}>
          <path d={`M40 ${cy} C 100 ${cy - spread}, 220 ${cy - spread}, 280 ${cy} C 220 ${cy + spread}, 100 ${cy + spread}, 40 ${cy}`} opacity="0.5" strokeWidth="1.1" />
          <path d={`M40 ${cy} C 90 ${cy - spread * 0.62}, 230 ${cy - spread * 0.62}, 280 ${cy}`} opacity="0.34" strokeWidth="1" />
          <path className="flowline" d={`M40 ${cy} C 100 ${cy - spread}, 220 ${cy - spread}, 280 ${cy}`} opacity="0.7" strokeWidth="1.4" />
          <path d={`M40 ${cy} C 90 ${cy + spread * 0.62}, 230 ${cy + spread * 0.62}, 280 ${cy}`} opacity="0.34" strokeWidth="1" />
          <circle cx="40" cy={cy} r="3.5" fill="currentColor" stroke="none" opacity="0.8" />
          <circle cx="280" cy={cy} r="3.5" fill="currentColor" stroke="none" opacity="0.8" />
        </g>
      </svg>
      <span className="brand-tile-mark">{initials || '◈'}</span>
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

  // Free vs paid is the only access model (see GameDetail's identical rule):
  // a game is free unless its developer set a positive per-session fee.
  // Showing "0 USDC" for a free game would misstate it as priced.
  const feePerSession = Number(game.fee_per_session ?? game.feePerSession);
  const isPaid = Number.isFinite(feePerSession) && feePerSession > 0
    && !(game.is_free ?? game.isFree);

  // Real thumbnail only — never a random stock photo. When a game has no
  // artwork (or the image fails to load) the neutral ErrorState tile renders.
  const thumbnailSrc = imageError ? null : (game.thumbnail || null);

  return (
    <article className="game-card spot sheen" aria-label={game.title}>
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
            <BrandTile seedText={`${game.title || ''} ${game.developer || ''}`} />
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
          {isPaid ? (
            <span className="price" role="img" aria-label={`${feePerSession} USD per session`}>
              <span className="price-value" aria-hidden="true">{feePerSession}</span>
              <span className="price-currency" aria-hidden="true">USDC</span>
              <span className="price-period" aria-hidden="true">/{t('games.sessions')}</span>
            </span>
          ) : (
            <span className="price price-free">Free to play</span>
          )}
        </div>
      </div>
    </article>
  );
});

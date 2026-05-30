import './StreamCard.css';

/**
 * StreamCard — displays a single live stream in the browse grid.
 * Props:
 *   stream  { id, title, game, streamer, viewerCount, thumbnailUrl, liveAt }
 *   onWatch (stream) => void
 */
export default function StreamCard({ stream, onWatch }) {
  const {
    title = 'Untitled Stream',
    game = 'Unknown Game',
    streamer = 'Anonymous',
    viewerCount = 0,
    thumbnailUrl,
  } = stream ?? {};

  const formattedViewers =
    viewerCount >= 1000
      ? `${(viewerCount / 1000).toFixed(1)}k`
      : String(viewerCount);

  return (
    <article
      className="stream-card"
      onClick={() => onWatch?.(stream)}
      role="button"
      tabIndex={0}
      aria-label={`Watch ${streamer} playing ${game}: ${title}. ${formattedViewers} viewers`}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onWatch?.(stream);
        }
      }}
    >
      {/* Thumbnail */}
      <div className="stream-card__thumb-wrap" aria-hidden="true">
        {thumbnailUrl ? (
          <img
            src={thumbnailUrl}
            alt=""
            className="stream-card__thumb-img"
            loading="lazy"
          />
        ) : (
          <div className="stream-card__thumb-placeholder">
            <svg
              width="36"
              height="36"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <polygon points="23 7 16 12 23 17 23 7" />
              <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
            </svg>
          </div>
        )}

        {/* Live badge */}
        <span className="stream-card__live-badge" aria-label="Live">
          <span className="stream-card__live-dot" aria-hidden="true" />
          LIVE
        </span>

        {/* Viewer count overlay */}
        <span className="stream-card__viewer-chip" aria-hidden="true">
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z" />
          </svg>
          {formattedViewers}
        </span>

        {/* Hover overlay */}
        <div className="stream-card__hover-overlay" aria-hidden="true">
          <svg
            width="40"
            height="40"
            viewBox="0 0 24 24"
            fill="white"
            aria-hidden="true"
          >
            <polygon points="5 3 19 12 5 21 5 3" />
          </svg>
        </div>
      </div>

      {/* Info */}
      <div className="stream-card__info">
        <div className="stream-card__avatar" aria-hidden="true">
          {(streamer ?? 'A').charAt(0).toUpperCase()}
        </div>
        <div className="stream-card__text">
          <p className="stream-card__title">{title}</p>
          <p className="stream-card__streamer">{streamer}</p>
          <p className="stream-card__game">{game}</p>
        </div>
      </div>
    </article>
  );
}

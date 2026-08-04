import { useState, useRef, useEffect, memo } from 'react';
import './GameScreenshot.css';

export default memo(function GameScreenshot({ src, alt, onClick, index }) {
  const [isLoaded, setIsLoaded] = useState(false);
  const [isInView, setIsInView] = useState(false);
  const imgRef                  = useRef(null);

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsInView(true);
          observer.disconnect();
        }
      },
      { rootMargin: '100px' }
    );

    if (imgRef.current) observer.observe(imgRef.current);

    return () => observer.disconnect();
  }, []);

  return (
    <button
      ref={imgRef}
      className="game-screenshot-thumbnail"
      onClick={() => onClick(index)}
      aria-label={`View screenshot ${index + 1}`}
    >
      <div className="thumbnail-placeholder">
        {!isLoaded && <div className="thumbnail-loading" aria-hidden="true" />}
      </div>

      {isInView && (
        <img
          src={src}
          alt={alt}
          className={`thumbnail-image ${isLoaded ? 'loaded' : ''}`}
          onLoad={() => setIsLoaded(true)}
          loading="lazy"
        />
      )}

      <div className="thumbnail-overlay" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35M11 8v6M8 11h6" />
        </svg>
      </div>
    </button>
  );
});

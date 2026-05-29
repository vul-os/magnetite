import { useState, useEffect, useCallback, useRef, memo } from 'react';
import Modal from './Modal';
import GameScreenshot from './GameScreenshot';

export default memo(function GameGallery({ images, title, initialIndex = 0 }) {
  const [isOpen, setIsOpen] = useState(false);
  const [currentIndex, setCurrentIndex] = useState(initialIndex);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [isZoomed, setIsZoomed] = useState(false);
  const touchStartX = useRef(null);
  const touchStartY = useRef(null);
  const galleryRef = useRef(null);

  const openGallery = useCallback((index) => {
    setCurrentIndex(index);
    setIsOpen(true);
    setIsZoomed(false);
  }, []);

  const closeGallery = useCallback(() => {
    setIsOpen(false);
    setIsFullscreen(false);
    setIsZoomed(false);
  }, []);

  const goToPrevious = useCallback(() => {
    setCurrentIndex((prev) => (prev === 0 ? images.length - 1 : prev - 1));
    setIsZoomed(false);
  }, [images.length]);

  const goToNext = useCallback(() => {
    setCurrentIndex((prev) => (prev === images.length - 1 ? 0 : prev + 1));
    setIsZoomed(false);
  }, [images.length]);

  const toggleFullscreen = useCallback(() => {
    setIsFullscreen((prev) => !prev);
  }, []);

  const toggleZoom = useCallback(() => {
    setIsZoomed((prev) => !prev);
  }, []);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e) => {
      switch (e.key) {
        case 'ArrowLeft':
          goToPrevious();
          break;
        case 'ArrowRight':
          goToNext();
          break;
        case 'Escape':
          if (isFullscreen) {
            setIsFullscreen(false);
          } else {
            closeGallery();
          }
          break;
        case 'f':
        case 'F':
          toggleFullscreen();
          break;
        default:
          break;
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, goToPrevious, goToNext, closeGallery, isFullscreen, toggleFullscreen]);

  const handleTouchStart = (e) => {
    touchStartX.current = e.touches[0].clientX;
    touchStartY.current = e.touches[0].clientY;
  };

  const handleTouchMove = (e) => {
    if (!touchStartX.current || !touchStartY.current) return;

    const deltaX = e.touches[0].clientX - touchStartX.current;
    const deltaY = e.touches[0].clientY - touchStartY.current;

    if (Math.abs(deltaX) > Math.abs(deltaY) && Math.abs(deltaX) > 50) {
      if (deltaX > 0) {
        goToPrevious();
      } else {
        goToNext();
      }
      touchStartX.current = null;
      touchStartY.current = null;
    }
  };

  const handleTouchEnd = () => {
    touchStartX.current = null;
    touchStartY.current = null;
  };

  const renderMainImage = () => (
    <div
      className={`gallery-main ${isFullscreen ? 'fullscreen' : ''}`}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      ref={galleryRef}
    >
      <div className={`gallery-image-container ${isZoomed ? 'zoomed' : ''}`} onClick={toggleZoom}>
        <img
          src={images[currentIndex]}
          alt={`${title} - Screenshot ${currentIndex + 1}`}
          className="gallery-main-image"
          loading="lazy"
        />
      </div>

      <div className="gallery-controls">
        <button className="gallery-nav-btn prev" onClick={goToPrevious} aria-label="Previous image">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M15 18l-6-6 6-6" />
          </svg>
        </button>

        <div className="gallery-counter">
          {currentIndex + 1} / {images.length}
        </div>

        <button className="gallery-nav-btn next" onClick={goToNext} aria-label="Next image">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M9 18l6-6-6-6" />
          </svg>
        </button>
      </div>

      <div className="gallery-actions">
        <button className="gallery-action-btn" onClick={toggleZoom} aria-label={isZoomed ? 'Zoom out' : 'Zoom in'}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            {isZoomed ? (
              <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3" />
            ) : (
              <>
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.35-4.35M11 8v6M8 11h6" />
              </>
            )}
          </svg>
        </button>
        <button className="gallery-action-btn" onClick={toggleFullscreen} aria-label="Toggle fullscreen">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            {isFullscreen ? (
              <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3" />
            ) : (
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
            )}
          </svg>
        </button>
      </div>
    </div>
  );

  const renderThumbnails = () => (
    <div className="gallery-thumbnails">
      {images.map((src, idx) => (
        <GameScreenshot
          key={idx}
          src={src}
          alt={`${title} - Thumbnail ${idx + 1}`}
          index={idx}
          onClick={openGallery}
        />
      ))}
    </div>
  );

  if (images.length === 0) return null;

  if (images.length === 1) {
    return (
      <div className="game-gallery single">
        <GameScreenshot
          src={images[0]}
          alt={`${title} - Screenshot`}
          index={0}
          onClick={openGallery}
        />
        <Modal isOpen={isOpen} onClose={closeGallery} size="xl" showCloseButton>
          <div className="gallery-single-view">
            <img
              src={images[0]}
              alt={`${title} - Screenshot`}
              className="gallery-single-image"
              loading="lazy"
            />
          </div>
        </Modal>
      </div>
    );
  }

  return (
    <div className="game-gallery">
      {renderThumbnails()}
      <Modal isOpen={isOpen} onClose={closeGallery} size="xl" showCloseButton={!isFullscreen}>
        <div className={`gallery-modal-content ${isFullscreen ? 'fullscreen-mode' : ''}`}>
          {renderMainImage()}
          <div className="gallery-thumbnail-strip">
            {images.map((src, idx) => (
              <button
                key={idx}
                className={`thumbnail-strip-item ${idx === currentIndex ? 'active' : ''}`}
                onClick={() => setCurrentIndex(idx)}
                aria-label={`Go to screenshot ${idx + 1}`}
              >
                <img src={src} alt={`Thumbnail ${idx + 1}`} loading="lazy" />
              </button>
            ))}
          </div>
        </div>
      </Modal>
    </div>
  );
});

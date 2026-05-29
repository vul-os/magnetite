import Button from './common/Button';
import './ReadyButton.css';

export default function ReadyButton({
  isReady,
  isHost = false,
  onToggleReady,
  disabled = false,
}) {
  if (isHost) {
    return (
      <div className="ready-button-wrapper">
        <div className="host-info-btn" aria-label="Host cannot ready up">
          <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d="M10 11v4m0-4a2 2 0 100-4 2 2 0 000 4z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          Host — Start When Ready
        </div>
      </div>
    );
  }

  return (
    <div className="ready-button-wrapper">
      <Button
        variant={isReady ? 'primary' : 'secondary'}
        size="lg"
        onClick={() => { if (!disabled) onToggleReady(!isReady); }}
        isDisabled={disabled}
        className={`ready-button ${isReady ? 'is-ready' : ''}`}
        aria-pressed={isReady}
        aria-label={isReady ? 'Mark as not ready' : 'Mark as ready'}
      >
        {isReady ? (
          <>
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path d="M16.667 5L7.5 14.167 3.333 10" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            Ready!
          </>
        ) : (
          <>
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <circle cx="10" cy="10" r="7" stroke="currentColor" strokeWidth="2" />
            </svg>
            Click When Ready
          </>
        )}
      </Button>
    </div>
  );
}

import Button from './common/Button';

export default function ReadyButton({
  isReady,
  isHost = false,
  onToggleReady,
  disabled = false,
}) {
  const handleClick = () => {
    if (!disabled && !isHost) {
      onToggleReady(!isReady);
    }
  };

  if (isHost) {
    return (
      <div className="ready-button-wrapper">
        <Button
          variant="secondary"
          size="lg"
          disabled
          className="ready-button host-button"
        >
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <path d="M10 11v4m0-4a2 2 0 100-4 2 2 0 000 4z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          Host cannot ready up
        </Button>

        <style>{`
          .ready-button-wrapper {
            width: 100%;
          }
          .ready-button {
            width: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.5rem;
          }
          .host-button {
            background: rgba(251, 191, 36, 0.1);
            border-color: rgba(251, 191, 36, 0.3);
            color: #fbbf24;
          }
          .host-button:hover {
            background: rgba(251, 191, 36, 0.15);
          }
        `}</style>
      </div>
    );
  }

  return (
    <div className="ready-button-wrapper">
      <Button
        variant={isReady ? 'primary' : 'secondary'}
        size="lg"
        onClick={handleClick}
        disabled={disabled}
        className={`ready-button ${isReady ? 'is-ready' : ''}`}
      >
        {isReady ? (
          <>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <path d="M16.667 5L7.5 14.167 3.333 10" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            Ready!
          </>
        ) : (
          <>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <circle cx="10" cy="10" r="7" stroke="currentColor" strokeWidth="2" />
            </svg>
            Click when ready
          </>
        )}
      </Button>

      <style>{`
        .ready-button-wrapper {
          width: 100%;
        }
        .ready-button {
          width: 100%;
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 0.5rem;
          transition: all 0.2s ease;
        }
        .ready-button.is-ready {
          background: var(--color-success, #22c55e);
          border-color: var(--color-success, #22c55e);
          box-shadow: 0 0 20px rgba(34, 197, 94, 0.3);
        }
        .ready-button.is-ready:hover {
          background: #16a34a;
          border-color: #16a34a;
        }
        .ready-button:not(.is-ready) {
          background: rgba(255, 255, 255, 0.05);
          border-color: rgba(255, 255, 255, 0.2);
        }
        .ready-button:not(.is-ready):hover:not(:disabled) {
          background: rgba(255, 255, 255, 0.1);
          border-color: rgba(255, 255, 255, 0.3);
        }
      `}</style>
    </div>
  );
}
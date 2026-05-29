import { useState, useEffect } from 'react';
import Button from './common/Button';

export default function StartGameButton({
  allPlayersReady,
  playerCount,
  minPlayers = 1,
  onStartGame,
  disabled = false,
}) {
  const [countdown, setCountdown] = useState(null);
  const [isStarting, setIsStarting] = useState(false);

  const canStart = allPlayersReady && playerCount >= minPlayers && !disabled;

  useEffect(() => {
    if (countdown === null) return;

    if (countdown > 0) {
      const timer = setTimeout(() => {
        setCountdown(countdown - 1);
      }, 1000);
      return () => clearTimeout(timer);
    } else {
      setCountdown(null);
      setIsStarting(true);
      onStartGame?.();
    }
  }, [countdown, onStartGame]);

  const handleClick = () => {
    if (!canStart) return;

    if (countdown !== null) {
      setCountdown(null);
      return;
    }

    setCountdown(3);
  };

  const handleCancelCountdown = () => {
    setCountdown(null);
  };

  if (countdown !== null) {
    return (
      <div className="start-game-wrapper">
        <Button
          variant="danger"
          size="lg"
          onClick={handleCancelCountdown}
          className="start-button countdown-active"
        >
          <div className="countdown-display">
            <span className="countdown-number">{countdown}</span>
            <span className="countdown-text">Cancel</span>
          </div>
        </Button>

        <style>{`
          .start-game-wrapper {
            width: 100%;
          }
          .start-button {
            width: 100%;
            height: 56px;
            display: flex;
            align-items: center;
            justify-content: center;
          }
          .countdown-active {
            background: linear-gradient(135deg, rgba(239, 68, 68, 0.9), rgba(220, 38, 38, 0.9));
            border-color: #ef4444;
            animation: pulse 1s ease-in-out infinite;
          }
          @keyframes pulse {
            0%, 100% { box-shadow: 0 0 0 0 rgba(239, 68, 68, 0.4); }
            50% { box-shadow: 0 0 20px 5px rgba(239, 68, 68, 0.2); }
          }
          .countdown-display {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 0.25rem;
          }
          .countdown-number {
            font-size: 1.5rem;
            font-weight: 700;
            line-height: 1;
          }
          .countdown-text {
            font-size: 0.625rem;
            text-transform: uppercase;
            letter-spacing: 0.1em;
            opacity: 0.8;
          }
        `}</style>
      </div>
    );
  }

  if (isStarting) {
    return (
      <div className="start-game-wrapper">
        <Button
          variant="primary"
          size="lg"
          disabled
          loading
          className="start-button"
        >
          Starting Game...
        </Button>

        <style>{`
          .start-game-wrapper {
            width: 100%;
          }
          .start-button {
            width: 100%;
          }
        `}</style>
      </div>
    );
  }

  return (
    <div className="start-game-wrapper">
      <Button
        variant="primary"
        size="lg"
        onClick={handleClick}
        disabled={!canStart}
        className={`start-button ${canStart ? 'can-start' : ''}`}
      >
        {!canStart ? (
          <span className="start-disabled-text">
            {!allPlayersReady
              ? 'Waiting for all players to ready up'
              : playerCount < minPlayers
              ? `Need at least ${minPlayers} players`
              : 'Cannot start game'}
          </span>
        ) : (
          <>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <path d="M6 4l10 6-10 6V4z" fill="currentColor" />
            </svg>
            Start Game
          </>
        )}
      </Button>

      <style>{`
        .start-game-wrapper {
          width: 100%;
        }
        .start-button {
          width: 100%;
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 0.5rem;
          height: 56px;
          transition: all 0.2s ease;
        }
        .start-button.can-start {
          background: linear-gradient(135deg, var(--color-accent, #8b5cf6), #6366f1);
          border-color: var(--color-accent, #8b5cf6);
          box-shadow: 0 0 30px rgba(139, 92, 246, 0.4);
        }
        .start-button.can-start:hover {
          transform: translateY(-1px);
          box-shadow: 0 0 40px rgba(139, 92, 246, 0.5);
        }
        .start-button:not(.can-start) {
          background: rgba(255, 255, 255, 0.05);
          border-color: rgba(255, 255, 255, 0.1);
          color: var(--color-text-muted, #666);
        }
        .start-disabled-text {
          font-size: 0.875rem;
        }
      `}</style>
    </div>
  );
}
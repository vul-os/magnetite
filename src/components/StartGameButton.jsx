import { useState, useEffect, useRef } from 'react';
import Button from './common/Button';
import './StartGameButton.css';

export default function StartGameButton({
  allPlayersReady,
  playerCount,
  minPlayers = 1,
  onStartGame,
  disabled = false,
}) {
  const [countdown, setCountdown]   = useState(null);
  const [isStarting, setIsStarting] = useState(false);

  // Stable ref so the effect doesn't re-register when onStartGame identity changes
  const onStartGameRef = useRef(onStartGame);
  useEffect(() => { onStartGameRef.current = onStartGame; }, [onStartGame]);

  const canStart = allPlayersReady && playerCount >= minPlayers && !disabled;

  useEffect(() => {
    if (countdown === null) return;
    if (countdown <= 0) {
      // Use a timeout to avoid calling setState synchronously inside the effect body
      const t = setTimeout(() => {
        setCountdown(null);
        setIsStarting(true);
        onStartGameRef.current?.();
      }, 0);
      return () => clearTimeout(t);
    }
    const t = setTimeout(() => setCountdown(prev => (prev !== null ? prev - 1 : null)), 1000);
    return () => clearTimeout(t);
  }, [countdown]);

  const handleClick = () => {
    if (!canStart) return;
    if (countdown !== null) {
      setCountdown(null); // cancel
      return;
    }
    setCountdown(3);
  };

  if (countdown !== null) {
    return (
      <div className="start-game-wrapper">
        <Button
          variant="danger"
          size="lg"
          onClick={handleClick}
          className="start-button countdown-active"
          aria-label={`Starting in ${countdown} seconds. Click to cancel.`}
          aria-live="polite"
        >
          <div className="countdown-display">
            <span className="countdown-number">{countdown}</span>
            <span className="countdown-text">Cancel</span>
          </div>
        </Button>
      </div>
    );
  }

  if (isStarting) {
    return (
      <div className="start-game-wrapper">
        <Button
          variant="primary"
          size="lg"
          isDisabled
          isLoading
          className="start-button"
          aria-label="Starting game…"
        >
          Launching…
        </Button>
      </div>
    );
  }

  return (
    <div className="start-game-wrapper">
      <Button
        variant="primary"
        size="lg"
        onClick={handleClick}
        isDisabled={!canStart}
        className={`start-button ${canStart ? 'can-start' : ''}`}
        aria-disabled={!canStart}
        aria-label={canStart ? 'Start game' : !allPlayersReady ? 'Waiting for all players' : `Need at least ${minPlayers} players`}
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
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path d="M6 4l10 6-10 6V4z" fill="currentColor" />
            </svg>
            Start Game
          </>
        )}
      </Button>
    </div>
  );
}

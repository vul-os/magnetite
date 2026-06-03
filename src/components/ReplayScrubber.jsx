/**
 * ReplayScrubber — playback controls for a ReplayLog.
 *
 * Props:
 *   currentTick   number   — current tick position
 *   totalTicks    number   — total ticks in the replay
 *   playing       bool     — whether playback is running
 *   speed         number   — playback speed multiplier (0.25 | 0.5 | 1 | 2 | 4)
 *   onPlay        fn()     — toggle play/pause
 *   onSeek        fn(tick) — seek to a tick
 *   onSpeedChange fn(spd)  — change playback speed
 */

const SPEEDS = [0.25, 0.5, 1, 2, 4];

export default function ReplayScrubber({
  currentTick = 0,
  totalTicks = 0,
  playing = false,
  speed = 1,
  onPlay,
  onSeek,
  onSpeedChange,
}) {
  const pct = totalTicks > 0 ? Math.min(100, (currentTick / totalTicks) * 100) : 0;

  function handleRangeChange(e) {
    const tick = Math.round((e.target.value / 100) * totalTicks);
    onSeek?.(tick);
  }

  return (
    <div className="rs-root" role="group" aria-label="Replay playback controls">
      {/* Progress bar */}
      <div className="rs-bar-wrap">
        <input
          type="range"
          className="rs-range"
          min={0}
          max={100}
          step={0.01}
          value={pct}
          onChange={handleRangeChange}
          aria-label="Replay position"
          aria-valuemin={0}
          aria-valuemax={totalTicks}
          aria-valuenow={currentTick}
          aria-valuetext={`Tick ${currentTick} of ${totalTicks}`}
        />
        <div className="rs-progress" style={{ width: `${pct}%` }} aria-hidden="true" />
      </div>

      {/* Controls row */}
      <div className="rs-controls">
        {/* Rewind to start */}
        <button
          className="rs-btn"
          onClick={() => onSeek?.(0)}
          aria-label="Rewind to start"
          title="Rewind"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M2 3v10M14 3L7 8l7 5V3z" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>

        {/* Play / Pause */}
        <button
          className="rs-btn rs-btn--play"
          onClick={onPlay}
          aria-label={playing ? 'Pause' : 'Play'}
          title={playing ? 'Pause' : 'Play'}
        >
          {playing ? (
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <rect x="4" y="3" width="3" height="10" rx="1" fill="currentColor" />
              <rect x="9" y="3" width="3" height="10" rx="1" fill="currentColor" />
            </svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M4 3l10 5-10 5V3z" fill="currentColor" />
            </svg>
          )}
        </button>

        {/* Skip to end */}
        <button
          className="rs-btn"
          onClick={() => onSeek?.(totalTicks)}
          aria-label="Skip to end"
          title="Skip to end"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M14 3v10M2 3l7 5-7 5V3z" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>

        {/* Tick counter */}
        <span className="rs-tick" aria-live="polite" aria-atomic="true">
          {currentTick.toLocaleString()} / {totalTicks.toLocaleString()}
        </span>

        {/* Speed selector */}
        <div className="rs-speeds" role="group" aria-label="Playback speed">
          {SPEEDS.map((s) => (
            <button
              key={s}
              className={`rs-speed-btn${speed === s ? ' active' : ''}`}
              onClick={() => onSpeedChange?.(s)}
              aria-pressed={speed === s}
              aria-label={`${s}× speed`}
            >
              {s}×
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

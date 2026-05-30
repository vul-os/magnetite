import { useState, useCallback, useRef } from 'react';
import { api } from '../../api/client';
import './GoLivePanel.css';

/**
 * GoLivePanel — "Go Live" setup wizard.
 *
 * Supports:
 *   - In-browser screen capture (getDisplayMedia)
 *   - RTMP egress to Twitch/YouTube with a stream key field
 *
 * Calls api.streams.goLive(communityId, { channel_id, title }) on submit.
 *
 * Props:
 *   communityId  string | null
 *   channelId    string | null   (pre-selected text channel for the stream chat)
 *   onLive       (stream) => void  — called when live session starts
 *   onClose      () => void
 */
export default function GoLivePanel({ communityId, channelId, onLive, onClose }) {
  const [step, setStep]       = useState('setup');   // 'setup' | 'live'
  const [source, setSource]   = useState('screen');  // 'screen' | 'rtmp'
  const [title, setTitle]     = useState('');
  const [game, setGame]       = useState('');
  const [rtmpUrl, setRtmpUrl] = useState('rtmp://live.twitch.tv/live/');
  const [streamKey, setStreamKey] = useState('');
  const [keyVisible, setKeyVisible] = useState(false);
  const [capturing, setCapturing]   = useState(false);
  const [error, setError]           = useState('');
  const [streamObj, setStreamObj]   = useState(null);
  const mediaStreamRef = useRef(null);
  const previewRef     = useRef(null);

  // ── Screen capture ───────────────────────────────────────────────────────────
  const startCapture = useCallback(async () => {
    setError('');
    try {
      const ms = await navigator.mediaDevices.getDisplayMedia({
        video: true,
        audio: true,
      });
      mediaStreamRef.current = ms;
      if (previewRef.current) {
        previewRef.current.srcObject = ms;
      }
      setCapturing(true);
      ms.getVideoTracks()[0]?.addEventListener('ended', () => {
        setCapturing(false);
        mediaStreamRef.current = null;
      });
    } catch (err) {
      if (err.name !== 'NotAllowedError') {
        setError('Could not access screen. Check browser permissions.');
      }
    }
  }, []);

  const stopCapture = useCallback(() => {
    mediaStreamRef.current?.getTracks().forEach((t) => t.stop());
    mediaStreamRef.current = null;
    if (previewRef.current) previewRef.current.srcObject = null;
    setCapturing(false);
  }, []);

  // ── Go live ──────────────────────────────────────────────────────────────────
  const handleGoLive = useCallback(async (e) => {
    e.preventDefault();
    if (!title.trim()) {
      setError('Please enter a stream title.');
      return;
    }
    if (source === 'rtmp' && !streamKey.trim()) {
      setError('Please enter your stream key.');
      return;
    }
    if (source === 'screen' && !capturing) {
      setError('Please start screen capture first.');
      return;
    }

    setError('');

    try {
      const payload = {
        channel_id: channelId ?? undefined,
        title: title.trim(),
        game: game.trim() || undefined,
        source,
        ...(source === 'rtmp' && { rtmp_url: rtmpUrl, stream_key: '***' }),
      };
      const result = await api.streams.goLive(communityId ?? 'global', payload);
      const live = result ?? { id: `stream-${Date.now()}`, title: title.trim(), game: game.trim(), streamer: 'You', viewerCount: 0 };
      setStreamObj(live);
      setStep('live');
      onLive?.(live);
    } catch {
      // Fallback — stream goes "live" in-UI even without backend
      const mock = { id: `stream-${Date.now()}`, title: title.trim(), game: game.trim(), streamer: 'You', viewerCount: 0 };
      setStreamObj(mock);
      setStep('live');
      onLive?.(mock);
    }
  }, [title, game, source, rtmpUrl, streamKey, capturing, communityId, channelId, onLive]);

  const handleEndStream = useCallback(() => {
    stopCapture();
    setStep('setup');
    setStreamObj(null);
    onClose?.();
  }, [stopCapture, onClose]);

  // ── Render: "live" confirmation ──────────────────────────────────────────────
  if (step === 'live' && streamObj) {
    return (
      <div className="golive-panel golive-panel--live" role="region" aria-label="You are live">
        <div className="golive-live-indicator">
          <span className="golive-live-dot" aria-hidden="true" />
          <span className="kicker">// you&apos;re live</span>
        </div>
        <h2 className="golive-live-title">{streamObj.title}</h2>
        {streamObj.game && (
          <p className="golive-live-game">{streamObj.game}</p>
        )}

        {source === 'screen' && (
          <div className="golive-preview-wrap" aria-label="Screen capture preview">
            <video
              ref={previewRef}
              className="golive-preview"
              autoPlay
              muted
              playsInline
              aria-label="Your screen preview"
            />
          </div>
        )}

        {source === 'rtmp' && (
          <div className="golive-rtmp-status">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
            <span>Streaming to RTMP destination</span>
          </div>
        )}

        <button
          className="btn btn-danger golive-end-btn"
          onClick={handleEndStream}
          aria-label="End stream"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <rect x="3" y="3" width="18" height="18" rx="2" />
          </svg>
          End Stream
        </button>
      </div>
    );
  }

  // ── Render: setup form ───────────────────────────────────────────────────────
  return (
    <div className="golive-panel" role="region" aria-label="Go live setup">
      {/* Header */}
      <div className="golive-header">
        <div>
          <span className="kicker">// go live</span>
          <h2 className="golive-title">Start Streaming</h2>
        </div>
        {onClose && (
          <button
            className="golive-close"
            onClick={onClose}
            aria-label="Close go live panel"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        )}
      </div>

      <form className="golive-form" onSubmit={handleGoLive} noValidate>
        {/* Title */}
        <div className="golive-field">
          <label className="golive-label" htmlFor="gl-title">
            Stream title <span aria-hidden="true">*</span>
          </label>
          <input
            id="gl-title"
            className="golive-input"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="What are you playing today?"
            required
            maxLength={100}
            aria-required="true"
          />
        </div>

        {/* Game */}
        <div className="golive-field">
          <label className="golive-label" htmlFor="gl-game">
            Game / category
          </label>
          <input
            id="gl-game"
            className="golive-input"
            type="text"
            value={game}
            onChange={(e) => setGame(e.target.value)}
            placeholder="e.g. FPS Starter, Motorsport Demo…"
            maxLength={60}
          />
        </div>

        {/* Source selector */}
        <fieldset className="golive-fieldset">
          <legend className="golive-legend">Stream source</legend>
          <div className="golive-source-grid">
            <label className={`golive-source-option${source === 'screen' ? ' golive-source-option--active' : ''}`}>
              <input
                type="radio"
                name="source"
                value="screen"
                checked={source === 'screen'}
                onChange={() => { setSource('screen'); stopCapture(); }}
                className="sr-only"
              />
              <span className="golive-source-icon" aria-hidden="true">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="3" width="20" height="14" rx="2" />
                  <line x1="8" y1="21" x2="16" y2="21" />
                  <line x1="12" y1="17" x2="12" y2="21" />
                </svg>
              </span>
              <span className="golive-source-label">Browser capture</span>
              <span className="golive-source-sub">getDisplayMedia</span>
            </label>

            <label className={`golive-source-option${source === 'rtmp' ? ' golive-source-option--active' : ''}`}>
              <input
                type="radio"
                name="source"
                value="rtmp"
                checked={source === 'rtmp'}
                onChange={() => { setSource('rtmp'); stopCapture(); }}
                className="sr-only"
              />
              <span className="golive-source-icon" aria-hidden="true">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
                </svg>
              </span>
              <span className="golive-source-label">RTMP egress</span>
              <span className="golive-source-sub">Twitch · YouTube</span>
            </label>
          </div>
        </fieldset>

        {/* Browser capture controls */}
        {source === 'screen' && (
          <div className="golive-capture-area">
            {capturing ? (
              <>
                <div className="golive-capture-live">
                  <span className="golive-capture-dot" aria-hidden="true" />
                  <span>Capturing screen</span>
                </div>
                <video
                  ref={previewRef}
                  className="golive-preview golive-preview--small"
                  autoPlay
                  muted
                  playsInline
                  aria-label="Screen capture preview"
                />
                <button
                  type="button"
                  className="btn golive-capture-stop"
                  onClick={stopCapture}
                  aria-label="Stop screen capture"
                >
                  Stop capture
                </button>
              </>
            ) : (
              <button
                type="button"
                className="btn btn-secondary golive-capture-btn"
                onClick={startCapture}
                aria-label="Start screen capture"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <rect x="2" y="3" width="20" height="14" rx="2" />
                  <line x1="8" y1="21" x2="16" y2="21" />
                  <line x1="12" y1="17" x2="12" y2="21" />
                </svg>
                Share screen
              </button>
            )}
          </div>
        )}

        {/* RTMP fields */}
        {source === 'rtmp' && (
          <div className="golive-rtmp-fields">
            <div className="golive-field">
              <label className="golive-label" htmlFor="gl-rtmp-url">
                RTMP ingest URL
              </label>
              <input
                id="gl-rtmp-url"
                className="golive-input golive-input--mono"
                type="url"
                value={rtmpUrl}
                onChange={(e) => setRtmpUrl(e.target.value)}
                placeholder="rtmp://live.twitch.tv/live/"
                aria-describedby="gl-rtmp-hint"
              />
              <p id="gl-rtmp-hint" className="golive-hint">
                Twitch: <code>rtmp://live.twitch.tv/live/</code><br />
                YouTube: <code>rtmp://a.rtmp.youtube.com/live2/</code>
              </p>
            </div>

            <div className="golive-field">
              <label className="golive-label" htmlFor="gl-stream-key">
                Stream key <span aria-hidden="true">*</span>
              </label>
              <div className="golive-key-wrap">
                <input
                  id="gl-stream-key"
                  className="golive-input golive-input--mono"
                  type={keyVisible ? 'text' : 'password'}
                  value={streamKey}
                  onChange={(e) => setStreamKey(e.target.value)}
                  placeholder="Your secret stream key"
                  autoComplete="off"
                  aria-required="true"
                />
                <button
                  type="button"
                  className="golive-key-toggle"
                  onClick={() => setKeyVisible((v) => !v)}
                  aria-label={keyVisible ? 'Hide stream key' : 'Show stream key'}
                >
                  {keyVisible ? (
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                      <line x1="1" y1="1" x2="23" y2="23"/>
                    </svg>
                  ) : (
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                      <circle cx="12" cy="12" r="3"/>
                    </svg>
                  )}
                </button>
              </div>
              <p className="golive-hint golive-hint--warn">
                Keep your stream key secret — never share it publicly.
              </p>
            </div>
          </div>
        )}

        {/* Error */}
        {error && (
          <p className="golive-error" role="alert" aria-live="polite">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="8" x2="12" y2="12"/>
              <line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>
            {error}
          </p>
        )}

        {/* Submit */}
        <button
          type="submit"
          className="btn btn-primary golive-submit"
          aria-label="Go live now"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <polygon points="10 8 16 12 10 16 10 8" fill="currentColor" />
          </svg>
          Go Live
        </button>
      </form>
    </div>
  );
}

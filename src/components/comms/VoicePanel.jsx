import { useState } from 'react';
import PresenceDot from './PresenceDot';
import './PresenceDot.css';
import './VoicePanel.css';

/**
 * VoicePanel — floating voice channel status bar.
 * Shows connected users, speaking indicators, mute/deafen/leave controls.
 * This is a visual shell; WebRTC wiring happens in Wave 7.
 */

function MicIcon({ muted }) {
  if (muted) {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <line x1="1" y1="1" x2="23" y2="23"/>
        <path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"/>
        <path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"/>
        <line x1="12" y1="19" x2="12" y2="23"/>
        <line x1="8" y1="23" x2="16" y2="23"/>
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
      <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
      <line x1="12" y1="19" x2="12" y2="23"/>
      <line x1="8" y1="23" x2="16" y2="23"/>
    </svg>
  );
}

function HeadsetIcon({ deafened }) {
  if (deafened) {
    return (
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <line x1="1" y1="1" x2="23" y2="23"/>
        <path d="M3 18v-6a9 9 0 0 1 15-6.7M21 18v-6a9 9 0 0 0-1.11-4.37"/>
        <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/>
      </svg>
    );
  }
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 18v-6a9 9 0 0 1 18 0v6"/>
      <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/>
    </svg>
  );
}

function PhoneOffIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M10.68 13.31a16 16 0 0 0 3.41 2.6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7 2 2 0 0 1 1.72 2v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07C9.44 17.25 8.76 16.57 8.1 15.9"/>
      <path d="M6.1 6.1A19.86 19.86 0 0 0 4.69 8.63 2 2 0 0 0 6.41 11a12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L5.39 17.2"/>
      <line x1="23" y1="1" x2="1" y2="23"/>
    </svg>
  );
}

export default function VoicePanel({ channel, participants = [], onLeave }) {
  const [muted,    setMuted]    = useState(false);
  const [deafened, setDeafened] = useState(false);

  if (!channel) return null;

  return (
    <div className="voice-panel" role="region" aria-label={`Voice: ${channel.name}`}>
      {/* Status line */}
      <div className="voice-panel__status">
        <span className="voice-panel__indicator" aria-hidden="true" />
        <div className="voice-panel__info">
          <span className="voice-panel__channel-name">
            {channel.name}
          </span>
          <span className="voice-panel__sub kicker">
            Voice Connected · {participants.length} connected
          </span>
        </div>
      </div>

      {/* Participant avatars */}
      {participants.length > 0 && (
        <div className="voice-panel__participants" aria-label="Voice participants">
          {participants.map((p) => (
            <div
              key={p.id}
              className={`voice-participant${p.speaking ? ' voice-participant--speaking' : ''}`}
              title={`${p.username}${p.muted ? ' (muted)' : ''}${p.speaking ? ' — speaking' : ''}`}
            >
              <div className="voice-participant__avatar" aria-hidden="true">
                <span className="voice-participant__initial">
                  {p.username?.charAt(0).toUpperCase() ?? 'U'}
                </span>
              </div>
              <PresenceDot status={p.status ?? 'online'} size="sm" className="voice-participant__dot" />
              {p.muted && (
                <span className="voice-participant__mute-icon" aria-label="Muted">
                  <MicIcon muted />
                </span>
              )}
              {p.speaking && (
                <span className="voice-participant__speaking-ring" aria-hidden="true" />
              )}
              <span className="voice-participant__name">{p.username}</span>
            </div>
          ))}
        </div>
      )}

      {/* Controls */}
      <div className="voice-panel__controls" aria-label="Voice controls">
        <button
          className={`voice-ctrl-btn${muted ? ' voice-ctrl-btn--active' : ''}`}
          onClick={() => setMuted(v => !v)}
          aria-label={muted ? 'Unmute microphone' : 'Mute microphone'}
          aria-pressed={muted}
          title={muted ? 'Unmute' : 'Mute'}
        >
          <MicIcon muted={muted} />
          <span className="voice-ctrl-btn__label">{muted ? 'Unmute' : 'Mute'}</span>
        </button>

        <button
          className={`voice-ctrl-btn${deafened ? ' voice-ctrl-btn--active' : ''}`}
          onClick={() => setDeafened(v => !v)}
          aria-label={deafened ? 'Undeafen' : 'Deafen'}
          aria-pressed={deafened}
          title={deafened ? 'Undeafen' : 'Deafen'}
        >
          <HeadsetIcon deafened={deafened} />
          <span className="voice-ctrl-btn__label">{deafened ? 'Undeafen' : 'Deafen'}</span>
        </button>

        <button
          className="voice-ctrl-btn voice-ctrl-btn--leave"
          onClick={onLeave}
          aria-label="Leave voice channel"
          title="Leave voice"
        >
          <PhoneOffIcon />
          <span className="voice-ctrl-btn__label">Leave</span>
        </button>
      </div>
    </div>
  );
}

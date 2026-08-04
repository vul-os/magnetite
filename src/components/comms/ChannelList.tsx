import './ChannelList.css';

/**
 * ChannelList — second column showing text + voice channel groups for the active server.
 * Renders two sections: TEXT CHANNELS and VOICE CHANNELS.
 * onSelect(channel) is called when a channel is clicked.
 */

function HashIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true" focusable="false">
      <path d="M6.5 2.5L5 13.5M11 2.5L9.5 13.5M2.5 6h11M2 10h11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
    </svg>
  );
}

function VolumeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
      <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
      <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
    </svg>
  );
}

function LockIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
      <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
    </svg>
  );
}

export default function ChannelList({ server, channels, activeChannelId, onSelect, loading = false }) {
  // Normalise: API uses `kind`, mock uses `type`
  const normalised = channels.map(c => ({ ...c, type: c.type ?? c.kind ?? 'text' }));
  const textChannels  = normalised.filter(c => c.type === 'text');
  const voiceChannels = normalised.filter(c => c.type === 'voice');

  return (
    <aside className="channel-list" aria-label={`${server?.name ?? 'Server'} channels`}>
      {/* Server header */}
      <div className="channel-list__server-header">
        <h2 className="channel-list__server-name">{server?.name ?? 'Server'}</h2>
        <button className="channel-list__settings-btn" aria-label="Server settings">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="3"/>
            <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
          </svg>
        </button>
      </div>

      <div className="channel-list__body">
        {/* Text channels section */}
        <section aria-labelledby="text-channels-heading">
          <div className="channel-section-header">
            <button
              className="channel-section-toggle"
              aria-expanded="true"
              id="text-channels-heading"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
                <path d="M2 3.5L5 6.5L8 3.5"/>
              </svg>
              TEXT CHANNELS
            </button>
            <button className="channel-add-btn" aria-label="Add text channel" title="Add text channel">
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                <line x1="6" y1="1" x2="6" y2="11"/><line x1="1" y1="6" x2="11" y2="6"/>
              </svg>
            </button>
          </div>

          <ul className="channel-items" role="list">
            {textChannels.map(ch => (
              <li key={ch.id} role="listitem">
                <button
                  className={`channel-item${ch.id === activeChannelId ? ' channel-item--active' : ''}${ch.unread ? ' channel-item--unread' : ''}`}
                  onClick={() => onSelect(ch)}
                  aria-current={ch.id === activeChannelId ? 'true' : undefined}
                  aria-label={`${ch.name}${ch.private ? ' (private)' : ''}${ch.unread ? `, ${ch.unread} unread` : ''}`}
                >
                  <span className="channel-item__icon" aria-hidden="true"><HashIcon /></span>
                  <span className="channel-item__name">{ch.name}</span>
                  {ch.private && <span className="channel-item__lock" aria-hidden="true"><LockIcon /></span>}
                  {ch.unread > 0 && ch.id !== activeChannelId && (
                    <span className="channel-item__badge" aria-hidden="true">{ch.unread > 9 ? '9+' : ch.unread}</span>
                  )}
                </button>
              </li>
            ))}
            {loading && textChannels.length === 0 && (
              <li role="listitem" aria-busy="true">
                <div className="channel-item-skeleton shimmer" aria-hidden="true" />
              </li>
            )}
          </ul>
        </section>

        {/* Voice channels section */}
        <section aria-labelledby="voice-channels-heading">
          <div className="channel-section-header">
            <button
              className="channel-section-toggle"
              aria-expanded="true"
              id="voice-channels-heading"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
                <path d="M2 3.5L5 6.5L8 3.5"/>
              </svg>
              VOICE CHANNELS
            </button>
            <button className="channel-add-btn" aria-label="Add voice channel" title="Add voice channel">
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                <line x1="6" y1="1" x2="6" y2="11"/><line x1="1" y1="6" x2="11" y2="6"/>
              </svg>
            </button>
          </div>

          <ul className="channel-items" role="list">
            {voiceChannels.map(ch => (
              <li key={ch.id} role="listitem">
                <button
                  className={`channel-item channel-item--voice${ch.id === activeChannelId ? ' channel-item--active' : ''}`}
                  onClick={() => onSelect(ch)}
                  aria-current={ch.id === activeChannelId ? 'true' : undefined}
                  aria-label={`${ch.name} voice channel${ch.participants?.length ? `, ${ch.participants.length} connected` : ''}`}
                >
                  <span className="channel-item__icon" aria-hidden="true"><VolumeIcon /></span>
                  <span className="channel-item__name">{ch.name}</span>
                  {ch.participants?.length > 0 && (
                    <span className="channel-item__count" aria-hidden="true">
                      {ch.participants.length}
                    </span>
                  )}
                </button>
                {/* Inline participants for active voice channel */}
                {ch.participants?.length > 0 && (
                  <ul className="voice-participants-inline" aria-label={`Users in ${ch.name}`}>
                    {ch.participants.map(p => (
                      <li key={p.id} className="voice-participant-inline">
                        <span className="voice-participant-inline__avatar" aria-hidden="true">
                          {p.username?.charAt(0).toUpperCase()}
                        </span>
                        <span className="voice-participant-inline__name">{p.username}</span>
                        {p.muted && (
                          <span className="voice-participant-inline__muted" aria-label="Muted">
                            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" aria-hidden="true">
                              <line x1="1" y1="1" x2="23" y2="23"/><path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"/>
                              <path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"/>
                              <line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>
                            </svg>
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </section>
      </div>

      {/* User controls strip at bottom */}
      <div className="channel-list__user-strip">
        <div className="user-strip__avatar" aria-hidden="true">Y</div>
        <div className="user-strip__info">
          <span className="user-strip__name">You</span>
          <span className="user-strip__status kicker">online</span>
        </div>
        <div className="user-strip__controls">
          <button className="strip-btn" aria-label="Mute microphone">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
              <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
              <line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>
            </svg>
          </button>
          <button className="strip-btn" aria-label="Deafen headset">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M3 18v-6a9 9 0 0 1 18 0v6"/>
              <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"/>
            </svg>
          </button>
          <button className="strip-btn" aria-label="User settings">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="3"/>
              <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
            </svg>
          </button>
        </div>
      </div>
    </aside>
  );
}

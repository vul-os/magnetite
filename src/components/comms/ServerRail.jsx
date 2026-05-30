import { useState } from 'react';
import './ServerRail.css';

/**
 * ServerRail — left column of circular server icons with magnetic hover +
 * active pill indicator. Pure visual shell; selection callback hoisted up.
 */
export default function ServerRail({ servers, activeId, onSelect }) {
  const [hovered, setHovered] = useState(null);

  return (
    <nav
      className="server-rail"
      aria-label="Communities (servers)"
      role="navigation"
    >
      {/* Home / DMs button */}
      <button
        className={`server-icon server-icon--home${activeId === '__home' ? ' server-icon--active' : ''}`}
        onClick={() => onSelect('__home')}
        aria-label="Direct Messages"
        aria-current={activeId === '__home' ? 'true' : undefined}
        title="Direct Messages"
      >
        <span aria-hidden="true">✉</span>
      </button>

      <div className="server-rail__divider" role="separator" aria-hidden="true" />

      {servers.map((server) => {
        const isActive  = activeId === server.id;
        const isHovered = hovered === server.id;

        return (
          <div key={server.id} className="server-icon-wrapper">
            {/* Active / hover pill */}
            <div
              className={`server-pill${isActive ? ' server-pill--active' : isHovered ? ' server-pill--hover' : ''}`}
              aria-hidden="true"
            />

            <button
              className={`server-icon${isActive ? ' server-icon--active' : ''}`}
              style={server.color ? { '--server-color': server.color } : undefined}
              onClick={() => onSelect(server.id)}
              onMouseEnter={() => setHovered(server.id)}
              onMouseLeave={() => setHovered(null)}
              aria-label={server.name}
              aria-current={isActive ? 'true' : undefined}
              title={server.name}
            >
              {server.icon ? (
                <img src={server.icon} alt="" aria-hidden="true" className="server-icon__img" />
              ) : (
                <span className="server-icon__abbr" aria-hidden="true">
                  {server.name.slice(0, 2).toUpperCase()}
                </span>
              )}

              {/* Unread badge */}
              {server.unread > 0 && !isActive && (
                <span className="server-badge" aria-label={`${server.unread} unread`}>
                  {server.unread > 9 ? '9+' : server.unread}
                </span>
              )}
            </button>
          </div>
        );
      })}

      {/* Add server */}
      <button
        className="server-icon server-icon--add"
        aria-label="Add a community"
        title="Add Community"
      >
        <span aria-hidden="true">+</span>
      </button>
    </nav>
  );
}

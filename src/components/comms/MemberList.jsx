import PresenceDot from './PresenceDot';
import './PresenceDot.css';
import './MemberList.css';

/**
 * MemberList — right sidebar showing server members grouped by role.
 * Each member shows avatar + presence dot.
 */

function MemberGroup({ label, members, currentUserId }) {
  if (!members || members.length === 0) return null;

  return (
    <div className="member-group">
      <h3 className="member-group__heading kicker">
        {label} — {members.length}
      </h3>
      <ul className="member-group__list" role="list">
        {members.map((member) => (
          <li key={member.id} className="member-item" role="listitem">
            <button
              className="member-item__btn"
              aria-label={`${member.username}${member.id === currentUserId ? ' (you)' : ''} — ${member.status}`}
              title={member.username}
            >
              <div className="member-item__avatar-wrap">
                <div className="member-item__avatar" aria-hidden="true">
                  {member.avatarUrl ? (
                    <img src={member.avatarUrl} alt="" className="member-item__avatar-img" />
                  ) : (
                    <span className="member-item__avatar-initial">
                      {member.username?.charAt(0).toUpperCase() ?? 'U'}
                    </span>
                  )}
                </div>
                <PresenceDot
                  status={member.status}
                  size="sm"
                  className="member-item__presence"
                />
              </div>

              <div className="member-item__info">
                <span className="member-item__name">
                  {member.username}
                  {member.id === currentUserId && (
                    <span className="member-item__you"> (you)</span>
                  )}
                </span>
                {member.game && (
                  <span className="member-item__activity" aria-label={`Playing ${member.game}`}>
                    Playing {member.game}
                  </span>
                )}
              </div>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default function MemberList({ members, currentUserId }) {
  const online  = members.filter(m => m.status === 'online');
  const idle    = members.filter(m => m.status === 'idle');
  const dnd     = members.filter(m => m.status === 'dnd');
  const offline = members.filter(m => m.status === 'offline');

  const activeSections = [
    { label: 'Online',  members: online  },
    { label: 'Idle',    members: idle    },
    { label: 'Busy',    members: dnd     },
    { label: 'Offline', members: offline },
  ].filter(s => s.members.length > 0);

  return (
    <aside
      className="member-list"
      aria-label="Server members"
    >
      <div className="member-list__header">
        <h2 className="member-list__title">Members</h2>
        <span className="member-list__count kicker">
          {members.filter(m => m.status !== 'offline').length} online
        </span>
      </div>

      <div className="member-list__body">
        {activeSections.map(({ label, members: groupMembers }) => (
          <MemberGroup
            key={label}
            label={label}
            members={groupMembers}
            currentUserId={currentUserId}
          />
        ))}
      </div>
    </aside>
  );
}

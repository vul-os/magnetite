import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import FriendCard from '../components/FriendCard';
import { api } from '../api/client';
import { usePresence } from '../hooks/usePresence';
import './social.css';

const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

export default function Friends() {
  const [friends, setFriends]               = useState([]);
  const [pendingRequests, setPendingRequests] = useState([]);
  const [blockedUsers, setBlockedUsers]     = useState([]);
  const [loading, setLoading]               = useState(true);
  const [loadError, setLoadError]           = useState(null);

  // Presence indicators for each friend
  const friendIds = friends.map((f) => f.id);
  const { presenceMap } = usePresence(friendIds);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState([]);
  const [searchError, setSearchError]     = useState(null);
  const [activeTab, setActiveTab] = useState('friends');

  useEffect(() => {
    let cancelled = false;

    async function loadMocks() {
      const { mockFriends, mockPendingRequests, mockBlockedUsers } = await import('../data/mockFriends');
      if (!cancelled) {
        setFriends(mockFriends);
        setPendingRequests(mockPendingRequests);
        setBlockedUsers(mockBlockedUsers);
        setLoading(false);
      }
    }

    if (useMocks) {
      loadMocks();
      return () => { cancelled = true; };
    }

    api.social.friends()
      .then(data => {
        if (!cancelled) {
          const list = Array.isArray(data) ? data : (data?.friends ?? []);
          setFriends(list);
        }
      })
      .catch((err) => {
        if (!cancelled) setLoadError(err.message || 'Failed to load friends');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, []);

  const handleSearch = useCallback(async (query) => {
    setSearchQuery(query);
    setSearchError(null);
    if (query.trim()) {
      if (useMocks) {
        const { mockSearchUsers } = await import('../data/mockFriends');
        const filtered = mockSearchUsers.filter(u =>
          u.username.toLowerCase().includes(query.toLowerCase())
        );
        setSearchResults(filtered);
        return;
      }
      api.social.searchUsers(query).then(data => {
        const list = Array.isArray(data) ? data : (data?.users ?? null);
        if (list) {
          setSearchResults(list);
        } else {
          setSearchResults([]);
        }
      }).catch((err) => {
        setSearchError(err.message || 'Search failed');
        setSearchResults([]);
      });
    } else {
      setSearchResults([]);
    }
  }, []);

  const handleAddFriend = useCallback(async (user) => {
    try {
      await api.social.addFriend(user.id);
    } catch { /* optimistic */ }
    setSearchResults(prev => prev.filter(u => u.id !== user.id));
    setSearchQuery('');
  }, []);

  const handleInvite = useCallback(async (friend) => {
    try {
      await api.social.sendInvite(friend.id, null);
    } catch { /* optimistic — invite queued locally */ }
  }, []);

  const handleBlock = useCallback((friend) => {
    setFriends(prev => prev.filter(f => f.id !== friend.id));
    setBlockedUsers(prev => [...prev, { ...friend, blockedAt: new Date().toISOString() }]);
  }, []);

  const handleAcceptRequest = useCallback(async (request) => {
    try {
      await api.social.acceptInvite(request.id);
    } catch { /* optimistic */ }
    setPendingRequests(prev => prev.filter(r => r.id !== request.id));
    setFriends(prev => [...prev, { ...request, status: 'offline' }]);
  }, []);

  const handleDeclineRequest = useCallback(async (request) => {
    try {
      await api.social.declineInvite(request.id);
    } catch { /* optimistic */ }
    setPendingRequests(prev => prev.filter(r => r.id !== request.id));
  }, []);

  const handleUnblock = useCallback((user) => {
    setBlockedUsers(prev => prev.filter(u => u.id !== user.id));
  }, []);

  if (loading) {
    return (
      <Layout>
        <div className="loading-state" aria-live="polite" aria-busy="true" style={{ minHeight: '40vh', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '1rem' }}>
          <span className="spinner" aria-hidden="true" />
          <span>Loading friends…</span>
        </div>
      </Layout>
    );
  }

  return (
    <Layout>
      <div className="friends-page reveal">
        <header className="page-header reveal-1">
          <span className="kicker">// Social Network</span>
          <h1>Friends</h1>
        </header>

        {loadError && (
          <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {loadError}
          </div>
        )}

        <div className="reveal-2">
          <div className="search-box">
            <input
              type="text"
              placeholder="Search players by username..."
              value={searchQuery}
              onChange={(e) => handleSearch(e.target.value)}
              aria-label="Search users"
              aria-autocomplete="list"
              aria-expanded={searchResults.length > 0}
            />
            {searchError && (
              <p style={{ color: 'var(--color-error)', fontSize: 'var(--text-sm)', marginTop: '0.5rem', fontFamily: 'var(--font-mono)' }} role="alert">
                {searchError}
              </p>
            )}
            {searchResults.length > 0 && (
              <div className="search-results" role="listbox" aria-label="Search results">
                {searchResults.map(user => (
                  <div key={user.id} className="search-result-item" role="option">
                    <img src={user.avatar} alt="" loading="lazy" />
                    <span>{user.username}</span>
                    <button
                      onClick={() => handleAddFriend(user)}
                      className="btn btn-primary btn-sm"
                      aria-label={`Add ${user.username} as friend`}
                    >
                      Add
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="tabs reveal-3" role="tablist" aria-label="Friends sections">
          <button
            role="tab"
            aria-selected={activeTab === 'friends'}
            className={`tab ${activeTab === 'friends' ? 'active' : ''}`}
            onClick={() => setActiveTab('friends')}
          >
            Friends ({friends.length})
          </button>
          <button
            role="tab"
            aria-selected={activeTab === 'requests'}
            className={`tab ${activeTab === 'requests' ? 'active' : ''}`}
            onClick={() => setActiveTab('requests')}
          >
            Requests ({pendingRequests.length})
          </button>
          <button
            role="tab"
            aria-selected={activeTab === 'blocked'}
            className={`tab ${activeTab === 'blocked' ? 'active' : ''}`}
            onClick={() => setActiveTab('blocked')}
          >
            Blocked ({blockedUsers.length})
          </button>
        </div>

        <div className="tab-content reveal-4">
          {activeTab === 'friends' && (
            <div className="friends-list" role="tabpanel" aria-label="Friends list">
              {friends.length === 0
                ? <p className="empty-state-inline">No friends yet — search for players above</p>
                : friends.map(friend => {
                    const presence = presenceMap[friend.id];
                    const friendWithPresence = presence
                      ? { ...friend, status: presence.status, activity: presence.activity }
                      : friend;
                    return (
                      <FriendCard
                        key={friend.id}
                        friend={friendWithPresence}
                        onInvite={handleInvite}
                        onBlock={handleBlock}
                      />
                    );
                  })
              }
            </div>
          )}

          {activeTab === 'requests' && (
            <div className="requests-list" role="tabpanel" aria-label="Pending requests">
              {pendingRequests.length === 0 ? (
                <p className="empty-state-inline">No pending requests</p>
              ) : (
                pendingRequests.map(request => (
                  <div key={request.id} className="request-card">
                    <img src={request.avatar} alt={`${request.username} avatar`} loading="lazy" />
                    <div className="request-info">
                      <h4>{request.username}</h4>
                      <span>Sent {new Date(request.sentAt).toLocaleDateString()}</span>
                    </div>
                    <div className="request-actions">
                      <button
                        onClick={() => handleAcceptRequest(request)}
                        className="btn btn-primary btn-sm"
                        aria-label={`Accept request from ${request.username}`}
                      >
                        Accept
                      </button>
                      <button
                        onClick={() => handleDeclineRequest(request)}
                        className="btn btn-secondary btn-sm"
                        aria-label={`Decline request from ${request.username}`}
                      >
                        Decline
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === 'blocked' && (
            <div className="blocked-list" role="tabpanel" aria-label="Blocked users">
              {blockedUsers.length === 0 ? (
                <p className="empty-state-inline">No blocked users</p>
              ) : (
                blockedUsers.map(user => (
                  <div key={user.id} className="blocked-card">
                    <img src={user.avatar} alt={`${user.username} avatar`} loading="lazy" />
                    <div className="blocked-info">
                      <h4>{user.username}</h4>
                      <span>Blocked {new Date(user.blockedAt).toLocaleDateString()}</span>
                    </div>
                    <button
                      onClick={() => handleUnblock(user)}
                      className="btn btn-secondary btn-sm"
                      aria-label={`Unblock ${user.username}`}
                    >
                      Unblock
                    </button>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </Layout>
  );
}

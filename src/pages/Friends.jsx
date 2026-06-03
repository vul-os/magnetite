import { useState, useEffect, useCallback } from 'react';
import Layout from '../components/Layout';
import FriendCard from '../components/FriendCard';
import { api } from '../api/client';
import { usePresence } from '../hooks/usePresence';
import { useTranslation } from '../i18n/useTranslation';
import './social.css';

const useMocks = import.meta.env.VITE_USE_MOCKS === 'true';

export default function Friends() {
  const { t } = useTranslation();
  const [friends, setFriends]               = useState([]);
  const [pendingRequests, setPendingRequests] = useState([]);
  const [sentRequests, setSentRequests]       = useState([]);
  const [blockedUsers, setBlockedUsers]       = useState([]);
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
        setSentRequests([]);
        setBlockedUsers(mockBlockedUsers);
        setLoading(false);
      }
    }

    if (useMocks) {
      loadMocks();
      return () => { cancelled = true; };
    }

    Promise.allSettled([
      api.social.friends(),
      api.social.pendingRequests(),
      api.social.sentRequests(),
    ]).then(([friendsResult, pendingResult, sentResult]) => {
      if (cancelled) return;

      if (friendsResult.status === 'fulfilled') {
        const data = friendsResult.value;
        const list = Array.isArray(data) ? data : (data?.friends ?? []);
        setFriends(list);
      } else {
        setLoadError(friendsResult.reason?.message || t('friends.loadError'));
      }

      if (pendingResult.status === 'fulfilled') {
        const data = pendingResult.value;
        const list = Array.isArray(data) ? data : (data?.requests ?? data?.friend_requests ?? []);
        setPendingRequests(list);
      }

      if (sentResult.status === 'fulfilled') {
        const data = sentResult.value;
        const list = Array.isArray(data) ? data : (data?.requests ?? data?.friend_requests ?? []);
        setSentRequests(list);
      }

      setLoading(false);
    }).catch((err) => {
      if (!cancelled) {
        setLoadError(err.message || t('friends.loadError'));
        setLoading(false);
      }
    });

    return () => { cancelled = true; };
  }, [t]);

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
        setSearchError(err.message || t('friends.searchError'));
        setSearchResults([]);
      });
    } else {
      setSearchResults([]);
    }
  }, [t]);

  const handleAddFriend = useCallback(async (user) => {
    try {
      await api.social.addFriend(user.id);
      // Move from search results to sent requests (optimistic)
      setSentRequests(prev => [...prev, {
        id: `pending_${user.id}`,
        from_user_id: 'me',
        to_user_id: user.id,
        username: user.username,
        avatar: user.avatar,
        status: 'pending',
        created_at: new Date().toISOString(),
        sentAt: new Date().toISOString(),
      }]);
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
      // Use the backend's accept route: POST /friends/accept/:id
      await api.social.acceptRequest(request.id).catch(() =>
        api.social.acceptInvite(request.id)
      );
    } catch { /* optimistic */ }
    setPendingRequests(prev => prev.filter(r => r.id !== request.id));
    setFriends(prev => [...prev, {
      id: request.from_user_id ?? request.id,
      username: request.username ?? request.from_username ?? 'User',
      avatar: request.avatar ?? request.from_avatar ?? null,
      status: 'offline',
    }]);
  }, []);

  const handleDeclineRequest = useCallback(async (request) => {
    try {
      await api.social.rejectRequest(request.id).catch(() =>
        api.social.declineInvite(request.id)
      );
    } catch { /* optimistic */ }
    setPendingRequests(prev => prev.filter(r => r.id !== request.id));
  }, []);

  const handleCancelSentRequest = useCallback(async (request) => {
    try {
      await api.social.cancelRequest(request.id);
    } catch { /* optimistic */ }
    setSentRequests(prev => prev.filter(r => r.id !== request.id));
  }, []);

  const handleUnblock = useCallback((user) => {
    setBlockedUsers(prev => prev.filter(u => u.id !== user.id));
  }, []);

  const incomingCount = pendingRequests.length;
  const sentCount = sentRequests.length;

  if (loading) {
    return (
      <Layout>
        <div className="loading-state" aria-live="polite" aria-busy="true" style={{ minHeight: '40vh', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '1rem' }}>
          <span className="spinner" aria-hidden="true" />
          <span>{t('friends.loading')}</span>
        </div>
      </Layout>
    );
  }

  return (
    <Layout>
      <div className="friends-page reveal">
        <header className="page-header reveal-1">
          <span className="kicker">// {t('friends.kicker')}</span>
          <h1>{t('friends.title')}</h1>
        </header>

        {loadError && (
          <div className="auth-error" role="alert" style={{ marginBottom: '1rem' }}>
            <span className="auth-error-icon" aria-hidden="true">!</span>
            {loadError}
          </div>
        )}

        <div className="reveal-2">
          <div className="search-box">
            <label htmlFor="friends-search" className="sr-only">{t('friends.searchLabel')}</label>
            <input
              id="friends-search"
              type="search"
              placeholder={t('friends.searchPlaceholder')}
              value={searchQuery}
              onChange={(e) => handleSearch(e.target.value)}
              aria-label={t('friends.searchLabel')}
              aria-autocomplete="list"
              aria-expanded={searchResults.length > 0}
              aria-controls={searchResults.length > 0 ? 'friends-search-results' : undefined}
            />
            {searchError && (
              <p style={{ color: 'var(--color-error)', fontSize: 'var(--text-sm)', marginTop: '0.5rem', fontFamily: 'var(--font-mono)' }} role="alert">
                {searchError}
              </p>
            )}
            {searchResults.length > 0 && (
              <div id="friends-search-results" className="search-results" role="listbox" aria-label={t('friends.searchResults')}>
                {searchResults.map(user => (
                  <div key={user.id} className="search-result-item" role="option">
                    <img src={user.avatar} alt="" loading="lazy" />
                    <span>{user.username}</span>
                    <button
                      onClick={() => handleAddFriend(user)}
                      className="btn btn-primary btn-sm"
                      aria-label={t('friends.addFriendLabel', { name: user.username })}
                    >
                      {t('friends.add')}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="tabs reveal-3" role="tablist" aria-label={t('friends.tabsLabel')}>
          <button
            id="tab-friends"
            role="tab"
            aria-selected={activeTab === 'friends'}
            aria-controls="panel-friends"
            className={`tab ${activeTab === 'friends' ? 'active' : ''}`}
            onClick={() => setActiveTab('friends')}
          >
            {t('friends.tab.friends')} ({friends.length})
          </button>
          <button
            id="tab-requests"
            role="tab"
            aria-selected={activeTab === 'requests'}
            aria-controls="panel-requests"
            className={`tab ${activeTab === 'requests' ? 'active' : ''}`}
            onClick={() => setActiveTab('requests')}
          >
            {t('friends.tab.incoming')} {incomingCount > 0 && <span className="badge-count" aria-label={t('friends.incomingCount', { count: incomingCount })}>{incomingCount}</span>}
          </button>
          <button
            id="tab-sent"
            role="tab"
            aria-selected={activeTab === 'sent'}
            aria-controls="panel-sent"
            className={`tab ${activeTab === 'sent' ? 'active' : ''}`}
            onClick={() => setActiveTab('sent')}
          >
            {t('friends.tab.sent')} ({sentCount})
          </button>
          <button
            id="tab-blocked"
            role="tab"
            aria-selected={activeTab === 'blocked'}
            aria-controls="panel-blocked"
            className={`tab ${activeTab === 'blocked' ? 'active' : ''}`}
            onClick={() => setActiveTab('blocked')}
          >
            {t('friends.tab.blocked')} ({blockedUsers.length})
          </button>
        </div>

        <div className="tab-content reveal-4">
          {activeTab === 'friends' && (
            <div id="panel-friends" className="friends-list" role="tabpanel" aria-labelledby="tab-friends">
              {friends.length === 0
                ? <p className="empty-state-inline">{t('friends.noFriends')}</p>
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
            <div id="panel-requests" className="requests-list" role="tabpanel" aria-labelledby="tab-requests">
              {pendingRequests.length === 0 ? (
                <p className="empty-state-inline">{t('friends.noIncoming')}</p>
              ) : (
                pendingRequests.map(request => (
                  <div key={request.id} className="request-card">
                    <img
                      src={request.avatar ?? request.from_avatar ?? `https://api.dicebear.com/7.x/identicon/svg?seed=${request.id}`}
                      alt={t('friends.avatarAlt', { name: request.username ?? request.from_username ?? 'User' })}
                      loading="lazy"
                    />
                    <div className="request-info">
                      <h4>{request.username ?? request.from_username ?? t('friends.unknownUser')}</h4>
                      <span>
                        {t('friends.sentDate', { date: new Date(request.created_at ?? request.sentAt ?? Date.now()).toLocaleDateString() })}
                      </span>
                    </div>
                    <div className="request-actions">
                      <button
                        onClick={() => handleAcceptRequest(request)}
                        className="btn btn-primary btn-sm"
                        aria-label={t('friends.acceptLabel', { name: request.username ?? request.from_username ?? t('friends.user') })}
                      >
                        {t('friends.accept')}
                      </button>
                      <button
                        onClick={() => handleDeclineRequest(request)}
                        className="btn btn-secondary btn-sm"
                        aria-label={t('friends.declineLabel', { name: request.username ?? request.from_username ?? t('friends.user') })}
                      >
                        {t('friends.decline')}
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === 'sent' && (
            <div id="panel-sent" className="requests-list" role="tabpanel" aria-labelledby="tab-sent">
              {sentRequests.length === 0 ? (
                <p className="empty-state-inline">{t('friends.noSent')}</p>
              ) : (
                sentRequests.map(request => (
                  <div key={request.id} className="request-card">
                    <img
                      src={request.avatar ?? request.to_avatar ?? `https://api.dicebear.com/7.x/identicon/svg?seed=${request.id}`}
                      alt={t('friends.avatarAlt', { name: request.username ?? request.to_username ?? 'User' })}
                      loading="lazy"
                    />
                    <div className="request-info">
                      <h4>{request.username ?? request.to_username ?? t('friends.unknownUser')}</h4>
                      <span>
                        {t('friends.sentDate', { date: new Date(request.created_at ?? request.sentAt ?? Date.now()).toLocaleDateString() })}
                      </span>
                    </div>
                    <div className="request-actions">
                      <button
                        onClick={() => handleCancelSentRequest(request)}
                        className="btn btn-secondary btn-sm"
                        aria-label={t('friends.cancelLabel', { name: request.username ?? request.to_username ?? t('friends.user') })}
                      >
                        {t('friends.cancel')}
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === 'blocked' && (
            <div id="panel-blocked" className="blocked-list" role="tabpanel" aria-labelledby="tab-blocked">
              {blockedUsers.length === 0 ? (
                <p className="empty-state-inline">{t('friends.noBlocked')}</p>
              ) : (
                blockedUsers.map(user => (
                  <div key={user.id} className="blocked-card">
                    <img src={user.avatar} alt={t('friends.avatarAlt', { name: user.username })} loading="lazy" />
                    <div className="blocked-info">
                      <h4>{user.username}</h4>
                      <span>{t('friends.blockedDate', { date: new Date(user.blockedAt).toLocaleDateString() })}</span>
                    </div>
                    <button
                      onClick={() => handleUnblock(user)}
                      className="btn btn-secondary btn-sm"
                      aria-label={t('friends.unblockLabel', { name: user.username })}
                    >
                      {t('friends.unblock')}
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

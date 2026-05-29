import { useState } from 'react';
import Layout from '../components/Layout';
import FriendCard from '../components/FriendCard';
import { mockFriends, mockPendingRequests, mockBlockedUsers, mockSearchUsers } from '../data/mockFriends';

export default function Friends() {
  const [friends, setFriends] = useState(mockFriends);
  const [pendingRequests, setPendingRequests] = useState(mockPendingRequests);
  const [blockedUsers, setBlockedUsers] = useState(mockBlockedUsers);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState([]);
  const [activeTab, setActiveTab] = useState('friends');
  const [showBlocked, setShowBlocked] = useState(false);

  const handleSearch = (query) => {
    setSearchQuery(query);
    if (query.trim()) {
      const filtered = mockSearchUsers.filter(u =>
        u.username.toLowerCase().includes(query.toLowerCase())
      );
      setSearchResults(filtered);
    } else {
      setSearchResults([]);
    }
  };

  const handleAddFriend = (user) => {
    setSearchResults(searchResults.filter(u => u.id !== user.id));
    setSearchQuery('');
    alert(`Friend request sent to ${user.username}`);
  };

  const handleInvite = (friend) => {
    alert(`Invite sent to ${friend.username}`);
  };

  const handleBlock = (friend) => {
    setFriends(friends.filter(f => f.id !== friend.id));
    setBlockedUsers([...blockedUsers, { ...friend, blockedAt: new Date().toISOString() }]);
    alert(`${friend.username} has been blocked`);
  };

  const handleAcceptRequest = (request) => {
    setPendingRequests(pendingRequests.filter(r => r.id !== request.id));
    setFriends([...friends, { ...request, status: 'offline' }]);
    alert(`You are now friends with ${request.username}`);
  };

  const handleDeclineRequest = (request) => {
    setPendingRequests(pendingRequests.filter(r => r.id !== request.id));
    alert(`Declined friend request from ${request.username}`);
  };

  const handleUnblock = (user) => {
    setBlockedUsers(blockedUsers.filter(u => u.id !== user.id));
    alert(`${user.username} has been unblocked`);
  };

  return (
    <Layout>
      <div className="friends-page">
        <header className="page-header">
          <h1>Friends</h1>
          <div className="search-box">
            <input
              type="text"
              placeholder="Search users..."
              value={searchQuery}
              onChange={(e) => handleSearch(e.target.value)}
            />
            {searchResults.length > 0 && (
              <div className="search-results">
                {searchResults.map(user => (
                  <div key={user.id} className="search-result-item">
                    <img src={user.avatar} alt={user.username} loading="lazy" />
                    <span>{user.username}</span>
                    <button onClick={() => handleAddFriend(user)} className="btn btn-primary btn-sm">
                      Add
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </header>

        <div className="tabs">
          <button
            className={`tab ${activeTab === 'friends' ? 'active' : ''}`}
            onClick={() => setActiveTab('friends')}
          >
            Friends ({friends.length})
          </button>
          <button
            className={`tab ${activeTab === 'requests' ? 'active' : ''}`}
            onClick={() => setActiveTab('requests')}
          >
            Requests ({pendingRequests.length})
          </button>
          <button
            className={`tab ${activeTab === 'blocked' ? 'active' : ''}`}
            onClick={() => setActiveTab('blocked')}
          >
            Blocked ({blockedUsers.length})
          </button>
        </div>

        <div className="tab-content">
          {activeTab === 'friends' && (
            <div className="friends-list">
              {friends.length === 0 ? (
                <p className="empty-state">No friends yet. Search for users to add friends!</p>
              ) : (
                friends.map(friend => (
                  <FriendCard
                    key={friend.id}
                    friend={friend}
                    onInvite={handleInvite}
                    onBlock={handleBlock}
                  />
                ))
              )}
            </div>
          )}

          {activeTab === 'requests' && (
            <div className="requests-list">
              {pendingRequests.length === 0 ? (
                <p className="empty-state">No pending requests</p>
              ) : (
                pendingRequests.map(request => (
                  <div key={request.id} className="request-card">
                    <img src={request.avatar} alt={request.username} loading="lazy" />
                    <div className="request-info">
                      <h4>{request.username}</h4>
                      <span>Sent {new Date(request.sentAt).toLocaleDateString()}</span>
                    </div>
                    <div className="request-actions">
                      <button onClick={() => handleAcceptRequest(request)} className="btn btn-primary btn-sm">
                        Accept
                      </button>
                      <button onClick={() => handleDeclineRequest(request)} className="btn btn-secondary btn-sm">
                        Decline
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}

          {activeTab === 'blocked' && (
            <div className="blocked-list">
              {blockedUsers.length === 0 ? (
                <p className="empty-state">No blocked users</p>
              ) : (
                blockedUsers.map(user => (
                  <div key={user.id} className="blocked-card">
                    <img src={user.avatar} alt={user.username} loading="lazy" />
                    <div className="blocked-info">
                      <h4>{user.username}</h4>
                      <span>Blocked {new Date(user.blockedAt).toLocaleDateString()}</span>
                    </div>
                    <button onClick={() => handleUnblock(user)} className="btn btn-secondary btn-sm">
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
import { useState, useRef, useEffect, useCallback } from 'react';
import { Link, useNavigate, useLocation } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { useWallet } from '../hooks/useWallet';
import {
  MenuIcon,
  CloseIcon,
  SearchIcon,
  BellIcon,
  ChevronDownIcon,
  HomeIcon,
  WalletIcon,
  SettingsIcon,
  LogoutIcon,
  TrophyIcon,
  UsersIcon,
} from '../assets/icons';
import './Navbar.css';

export default function Navbar() {
  const { user, logout } = useAuth();
  const { balance } = useWallet();
  const navigate = useNavigate();
  const location = useLocation();
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isUserMenuOpen, setIsUserMenuOpen] = useState(false);
  const [isNotificationsOpen, setIsNotificationsOpen] = useState(false);
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [isScrolled, setIsScrolled] = useState(false);
  const userMenuRef = useRef(null);
  const notificationsRef = useRef(null);
  const searchRef = useRef(null);

  const navLinks = [
    { path: '/', label: 'Marketplace', icon: HomeIcon },
    { path: '/developers', label: 'Developers', icon: UsersIcon },
    { path: '/leaderboard', label: 'Leaderboard', icon: TrophyIcon },
    { path: '/about', label: 'About', icon: UsersIcon },
  ];

  const notifications = [
    { id: 1, type: 'achievement', title: 'Achievement Unlocked', message: 'You earned the "Top Developer" badge', time: '2 hours ago', unread: true },
    { id: 2, type: 'payout', title: 'Payout Received', message: 'You received $50.00 USDC', time: '5 hours ago', unread: true },
    { id: 3, type: 'invite', title: 'Game Invite', message: 'PlayerX invited you to play GameY', time: '1 day ago', unread: false },
  ];

  const userMenuItems = [
    { path: '/profile', label: 'Profile', icon: UsersIcon },
    { path: '/settings', label: 'Settings', icon: SettingsIcon },
    { path: '/earnings', label: 'Earnings', icon: WalletIcon },
  ];

  useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 10);
    };
    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  useEffect(() => {
    function handleClickOutside(event) {
      if (userMenuRef.current && !userMenuRef.current.contains(event.target)) {
        setIsUserMenuOpen(false);
      }
      if (notificationsRef.current && !notificationsRef.current.contains(event.target)) {
        setIsNotificationsOpen(false);
      }
      if (searchRef.current && !searchRef.current.contains(event.target)) {
        setIsSearchOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    setIsMobileMenuOpen(false);
    setIsUserMenuOpen(false);
    setIsNotificationsOpen(false);
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [location.pathname]);

  const handleLogout = useCallback(() => {
    logout();
    navigate('/');
  }, [logout, navigate]);

  const handleSearch = useCallback((e) => {
    e.preventDefault();
    if (searchQuery.trim()) {
      navigate(`/marketplace?search=${encodeURIComponent(searchQuery)}`);
      setSearchQuery('');
      setIsSearchOpen(false);
    }
  }, [searchQuery, navigate]);

  const handleKeyDown = useCallback((e) => {
    if (e.key === 'Escape') {
      setIsSearchOpen(false);
      setIsUserMenuOpen(false);
      setIsNotificationsOpen(false);
    }
  }, []);

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  const unreadCount = notifications.filter(n => n.unread).length;

  return (
    <>
      <nav className={`navbar ${isScrolled ? 'scrolled' : ''}`}>
        <div className="navbar-container">
          <div className="navbar-left">
            <Link to="/home" className="navbar-logo">
              <div className="logo-icon">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>

            <div className="navbar-nav">
              {navLinks.map(({ path, label, icon: Icon }) => (
                <Link
                  key={path}
                  to={path}
                  className={`nav-link ${location.pathname === path ? 'active' : ''}`}
                >
                  <Icon className="nav-link-icon" />
                  <span>{label}</span>
                </Link>
              ))}
            </div>
          </div>

          <div className="navbar-right">
            <div ref={searchRef} className={`search-wrapper ${isSearchOpen ? 'open' : ''}`}>
              <button
                className="search-toggle"
                onClick={() => setIsSearchOpen(!isSearchOpen)}
                aria-label="Toggle search"
              >
                <SearchIcon />
              </button>
              <form onSubmit={handleSearch} className="search-form">
                <input
                  type="text"
                  placeholder="Search games..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="search-input"
                  autoFocus={isSearchOpen}
                />
              </form>
            </div>

            {user ? (
              <>
                <div className="wallet-balance">
                  <WalletIcon className="wallet-icon" />
                  <span className="balance-amount">${balance?.toFixed(2)}</span>
                  <span className="balance-currency">USDC</span>
                </div>

                <div ref={notificationsRef} className="notifications-wrapper">
                  <button
                    className="notifications-btn"
                    onClick={() => setIsNotificationsOpen(!isNotificationsOpen)}
                    aria-label="Notifications"
                    aria-expanded={isNotificationsOpen}
                  >
                    <BellIcon />
                    {unreadCount > 0 && (
                      <span className="notification-badge">{unreadCount}</span>
                    )}
                  </button>

                  {isNotificationsOpen && (
                    <div className="notification-dropdown">
                      <div className="dropdown-header">
                        <h3>Notifications</h3>
                        {unreadCount > 0 && (
                          <button className="mark-all-read">Mark all as read</button>
                        )}
                      </div>
                      <div className="notification-list">
                        {notifications.length > 0 ? (
                          notifications.map((notification) => (
                            <div
                              key={notification.id}
                              className={`notification-item ${notification.unread ? 'unread' : ''}`}
                            >
                              <div className={`notification-icon ${notification.type}`}>
                                <BellIcon />
                              </div>
                              <div className="notification-content">
                                <div className="notification-title">{notification.title}</div>
                                <div className="notification-message">{notification.message}</div>
                                <div className="notification-time">{notification.time}</div>
                              </div>
                              {notification.unread && <div className="unread-dot" />}
                            </div>
                          ))
                        ) : (
                          <div className="empty-state">
                            <BellIcon />
                            <p>No notifications yet</p>
                          </div>
                        )}
                      </div>
                    </div>
                  )}
                </div>

                <div ref={userMenuRef} className="user-menu-wrapper">
                  <button
                    className="user-menu-trigger"
                    onClick={() => setIsUserMenuOpen(!isUserMenuOpen)}
                    aria-expanded={isUserMenuOpen}
                    aria-label="User menu"
                  >
                    <div className="user-avatar">
                      {user.username?.charAt(0).toUpperCase() || 'U'}
                    </div>
                    <ChevronDownIcon className={`chevron ${isUserMenuOpen ? 'open' : ''}`} />
                  </button>

                  {isUserMenuOpen && (
                    <div className="user-dropdown">
                      <div className="dropdown-header">
                        <div className="dropdown-username">{user.username}</div>
                        <div className="dropdown-email">{user.email}</div>
                      </div>
                      <div className="dropdown-divider" />
                      {userMenuItems.map(({ path, label, icon: Icon }) => (
                        <Link
                          key={path}
                          to={path}
                          className="dropdown-item"
                          onClick={() => setIsUserMenuOpen(false)}
                        >
                          <Icon className="dropdown-icon" />
                          {label}
                        </Link>
                      ))}
                      <div className="dropdown-divider" />
                      <button
                        onClick={handleLogout}
                        className="dropdown-item logout"
                      >
                        <LogoutIcon className="dropdown-icon" />
                        Logout
                      </button>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="auth-buttons">
                <Link to="/login" className="btn btn-secondary">Login</Link>
                <Link to="/register" className="btn btn-primary">Register</Link>
              </div>
            )}

            <button
              className="mobile-menu-toggle"
              onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
              aria-label="Toggle menu"
              aria-expanded={isMobileMenuOpen}
            >
              {isMobileMenuOpen ? <CloseIcon /> : <MenuIcon />}
            </button>
          </div>
        </div>
      </nav>

      {isMobileMenuOpen && (
        <div
          className="mobile-overlay"
          onClick={() => setIsMobileMenuOpen(false)}
          aria-hidden="true"
        />
      )}

      <div
        className={`mobile-menu ${isMobileMenuOpen ? 'open' : ''}`}
        aria-hidden={!isMobileMenuOpen}
      >
        <div className="mobile-menu-header">
          <Link to="/" className="navbar-logo">
            <div className="logo-icon">M</div>
            <span className="logo-text">Magnetite</span>
          </Link>
          <button
            className="mobile-menu-close"
            onClick={() => setIsMobileMenuOpen(false)}
            aria-label="Close menu"
          >
            <CloseIcon />
          </button>
        </div>

        <div className="mobile-search">
          <form onSubmit={handleSearch}>
            <input
              type="text"
              placeholder="Search games..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="search-input"
            />
          </form>
        </div>

        <nav className="mobile-nav">
          {navLinks.map(({ path, label, icon: Icon }) => (
            <Link
              key={path}
              to={path}
              className={`mobile-nav-link ${location.pathname === path ? 'active' : ''}`}
            >
              <Icon className="mobile-nav-icon" />
              <span>{label}</span>
            </Link>
          ))}
        </nav>

        <div className="mobile-menu-footer">
          {user ? (
            <>
              <div className="mobile-user-info">
                <div className="mobile-user-avatar">
                  {user.username?.charAt(0).toUpperCase() || 'U'}
                </div>
                <div className="mobile-user-details">
                  <div className="mobile-username">{user.username}</div>
                  <div className="mobile-balance">${balance?.toFixed(2)} USDC</div>
                </div>
              </div>
              {userMenuItems.map(({ path, label, icon: Icon }) => (
                <Link
                  key={path}
                  to={path}
                  className="mobile-dropdown-item"
                >
                  <Icon className="mobile-dropdown-icon" />
                  {label}
                </Link>
              ))}
              <button onClick={handleLogout} className="mobile-dropdown-item logout">
                <LogoutIcon className="mobile-dropdown-icon" />
                Logout
              </button>
            </>
          ) : (
            <div className="mobile-auth-buttons">
              <Link to="/login" className="btn btn-secondary">Login</Link>
              <Link to="/register" className="btn btn-primary">Register</Link>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

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
  WalletIcon,
  SettingsIcon,
  LogoutIcon,
  TrophyIcon,
  UsersIcon,
  HomeIcon,
} from '../assets/icons';
import './Navbar.css';

const NAV_LINKS = [
  { path: '/',            label: 'Marketplace', icon: HomeIcon },
  { path: '/developers',  label: 'Developers',  icon: UsersIcon },
  { path: '/leaderboard', label: 'Leaderboard', icon: TrophyIcon },
  { path: '/about',       label: 'About',       icon: UsersIcon },
];

const USER_MENU_ITEMS = [
  { path: '/profile',  label: 'Profile',  icon: UsersIcon },
  { path: '/settings', label: 'Settings', icon: SettingsIcon },
  { path: '/earnings', label: 'Earnings', icon: WalletIcon },
];

// Sample notifications — in production, these come from the notification API
const SAMPLE_NOTIFICATIONS = [
  { id: 1, type: 'achievement', title: 'Achievement Unlocked', message: 'You earned the "Top Developer" badge', time: '2h ago', unread: true },
  { id: 2, type: 'payout',      title: 'Payout Received',      message: 'You received 50.00 USDC',             time: '5h ago', unread: true },
  { id: 3, type: 'invite',      title: 'Game Invite',           message: 'PlayerX invited you to join a lobby', time: '1d ago', unread: false },
];

export default function Navbar() {
  const { user, logout } = useAuth();
  const { balance } = useWallet();
  const navigate   = useNavigate();
  const location   = useLocation();

  const [isSearchOpen,        setIsSearchOpen]        = useState(false);
  const [isUserMenuOpen,      setIsUserMenuOpen]      = useState(false);
  const [isNotificationsOpen, setIsNotificationsOpen] = useState(false);
  const [isMobileMenuOpen,    setIsMobileMenuOpen]    = useState(false);
  const [searchQuery,         setSearchQuery]         = useState('');
  const [isScrolled,          setIsScrolled]          = useState(false);

  const userMenuRef       = useRef(null);
  const notificationsRef  = useRef(null);
  const searchRef         = useRef(null);

  // Scroll detection for glass-blur enhancement
  useEffect(() => {
    const onScroll = () => setIsScrolled(window.scrollY > 8);
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  // Click-outside to close dropdowns
  useEffect(() => {
    function onMouseDown(e) {
      if (userMenuRef.current      && !userMenuRef.current.contains(e.target))      setIsUserMenuOpen(false);
      if (notificationsRef.current && !notificationsRef.current.contains(e.target)) setIsNotificationsOpen(false);
      if (searchRef.current        && !searchRef.current.contains(e.target))        setIsSearchOpen(false);
    }
    document.addEventListener('mousedown', onMouseDown);
    return () => document.removeEventListener('mousedown', onMouseDown);
  }, []);

  // Close everything on route change
  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    setIsMobileMenuOpen(false);
    setIsUserMenuOpen(false);
    setIsNotificationsOpen(false);
    setIsSearchOpen(false);
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [location.pathname]);

  // Escape key
  useEffect(() => {
    function onKeyDown(e) {
      if (e.key === 'Escape') {
        setIsSearchOpen(false);
        setIsUserMenuOpen(false);
        setIsNotificationsOpen(false);
        setIsMobileMenuOpen(false);
      }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, []);

  const handleLogout = useCallback(() => {
    logout();
    navigate('/');
  }, [logout, navigate]);

  const handleSearch = useCallback((e) => {
    e.preventDefault();
    const q = searchQuery.trim();
    if (q) {
      navigate(`/marketplace?search=${encodeURIComponent(q)}`);
      setSearchQuery('');
      setIsSearchOpen(false);
    }
  }, [searchQuery, navigate]);

  const unreadCount = SAMPLE_NOTIFICATIONS.filter(n => n.unread).length;

  return (
    <>
      {/* ── Main bar ───────────────────────────────────────────────────────── */}
      <nav className={`navbar${isScrolled ? ' scrolled' : ''}`} role="navigation" aria-label="Main navigation">
        <div className="navbar-container">

          {/* Left: logo + nav links */}
          <div className="navbar-left">
            <Link to="/home" className="navbar-logo" aria-label="Magnetite home">
              <div className="logo-icon" aria-hidden="true">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>

            <nav className="navbar-nav" aria-label="Site sections">
              {NAV_LINKS.map(({ path, label }) => (
                <Link
                  key={path}
                  to={path}
                  className={`nav-link${location.pathname === path ? ' active' : ''}`}
                  aria-current={location.pathname === path ? 'page' : undefined}
                >
                  {label}
                </Link>
              ))}
            </nav>
          </div>

          {/* Right: search / wallet / notifications / user */}
          <div className="navbar-right">

            {/* Search */}
            <div ref={searchRef} className={`search-wrapper${isSearchOpen ? ' open' : ''}`}>
              <button
                className="search-toggle"
                onClick={() => setIsSearchOpen(v => !v)}
                aria-label="Open search"
                aria-expanded={isSearchOpen}
              >
                <SearchIcon />
              </button>
              <form onSubmit={handleSearch} className="search-form" role="search">
                <input
                  type="search"
                  placeholder="Search Rust games…"
                  value={searchQuery}
                  onChange={e => setSearchQuery(e.target.value)}
                  className="search-input"
                  aria-label="Search games"
                  autoFocus={isSearchOpen}
                />
              </form>
            </div>

            {user ? (
              <>
                {/* Wallet balance */}
                <Link to="/wallet" className="wallet-balance" aria-label={`Wallet: ${balance?.toFixed(2)} USDC`}>
                  <WalletIcon className="wallet-icon" aria-hidden="true" />
                  <span className="balance-amount">{balance?.toFixed(2) ?? '0.00'}</span>
                  <span className="balance-currency">USDC</span>
                </Link>

                {/* Notifications */}
                <div ref={notificationsRef} className="notifications-wrapper">
                  <button
                    className="notifications-btn"
                    onClick={() => setIsNotificationsOpen(v => !v)}
                    aria-label={`Notifications${unreadCount > 0 ? ` (${unreadCount} unread)` : ''}`}
                    aria-expanded={isNotificationsOpen}
                    aria-haspopup="true"
                  >
                    <BellIcon />
                    {unreadCount > 0 && (
                      <span className="notification-badge" aria-hidden="true">{unreadCount}</span>
                    )}
                  </button>

                  {isNotificationsOpen && (
                    <div className="notification-dropdown" role="dialog" aria-label="Notifications">
                      <div className="dropdown-header">
                        <h3>Notifications</h3>
                        {unreadCount > 0 && (
                          <button className="mark-all-read">Mark all read</button>
                        )}
                      </div>
                      <div className="notification-list">
                        {SAMPLE_NOTIFICATIONS.length > 0 ? (
                          SAMPLE_NOTIFICATIONS.map(n => (
                            <div
                              key={n.id}
                              className={`notification-item${n.unread ? ' unread' : ''}`}
                              role="article"
                            >
                              <div className={`notification-icon ${n.type}`} aria-hidden="true">
                                <BellIcon />
                              </div>
                              <div className="notification-content">
                                <div className="notification-title">{n.title}</div>
                                <div className="notification-message">{n.message}</div>
                                <time className="notification-time">{n.time}</time>
                              </div>
                              {n.unread && <div className="unread-dot" aria-label="Unread" />}
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

                {/* User menu */}
                <div ref={userMenuRef} className="user-menu-wrapper">
                  <button
                    className="user-menu-trigger"
                    onClick={() => setIsUserMenuOpen(v => !v)}
                    aria-expanded={isUserMenuOpen}
                    aria-haspopup="true"
                    aria-label={`User menu for ${user.username ?? 'account'}`}
                  >
                    <div className="user-avatar" aria-hidden="true">
                      {user.username?.charAt(0).toUpperCase() ?? 'U'}
                    </div>
                    <ChevronDownIcon className={`chevron${isUserMenuOpen ? ' open' : ''}`} aria-hidden="true" />
                  </button>

                  {isUserMenuOpen && (
                    <div className="user-dropdown" role="menu" aria-label="User options">
                      <div className="dropdown-header">
                        <div className="dropdown-username">{user.username}</div>
                        <div className="dropdown-email">{user.email}</div>
                      </div>
                      <div className="dropdown-divider" role="separator" />
                      {USER_MENU_ITEMS.map(({ path, label, icon: Icon }) => (
                        <Link
                          key={path}
                          to={path}
                          className="dropdown-item"
                          role="menuitem"
                          onClick={() => setIsUserMenuOpen(false)}
                        >
                          <Icon className="dropdown-icon" aria-hidden="true" />
                          {label}
                        </Link>
                      ))}
                      <div className="dropdown-divider" role="separator" />
                      <button
                        onClick={handleLogout}
                        className="dropdown-item logout"
                        role="menuitem"
                      >
                        <LogoutIcon className="dropdown-icon" aria-hidden="true" />
                        Log out
                      </button>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="auth-buttons">
                <Link to="/login"    className="btn btn-secondary">Log in</Link>
                <Link to="/register" className="btn btn-primary">Get started</Link>
              </div>
            )}

            {/* Mobile drawer toggle */}
            <button
              className="mobile-menu-toggle"
              onClick={() => setIsMobileMenuOpen(v => !v)}
              aria-label={isMobileMenuOpen ? 'Close menu' : 'Open menu'}
              aria-expanded={isMobileMenuOpen}
              aria-controls="mobile-nav-drawer"
            >
              {isMobileMenuOpen ? <CloseIcon /> : <MenuIcon />}
            </button>
          </div>
        </div>
      </nav>

      {/* ── Mobile backdrop ────────────────────────────────────────────────── */}
      {isMobileMenuOpen && (
        <div
          className="mobile-overlay"
          onClick={() => setIsMobileMenuOpen(false)}
          aria-hidden="true"
        />
      )}

      {/* ── Mobile drawer ──────────────────────────────────────────────────── */}
      <div
        id="mobile-nav-drawer"
        className={`mobile-menu${isMobileMenuOpen ? ' open' : ''}`}
        aria-hidden={!isMobileMenuOpen}
        role="dialog"
        aria-label="Mobile navigation"
      >
        <div className="mobile-menu-header">
          <Link to="/" className="navbar-logo" onClick={() => setIsMobileMenuOpen(false)}>
            <div className="logo-icon" aria-hidden="true">M</div>
            <span className="logo-text">Magnetite</span>
          </Link>
          <button
            className="mobile-menu-close"
            onClick={() => setIsMobileMenuOpen(false)}
            aria-label="Close navigation"
          >
            <CloseIcon />
          </button>
        </div>

        <div className="mobile-search">
          <form onSubmit={handleSearch} role="search">
            <input
              type="search"
              placeholder="Search Rust games…"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              className="search-input"
              aria-label="Search games"
            />
          </form>
        </div>

        <nav className="mobile-nav" aria-label="Mobile site sections">
          {NAV_LINKS.map(({ path, label, icon: Icon }) => (
            <Link
              key={path}
              to={path}
              className={`mobile-nav-link${location.pathname === path ? ' active' : ''}`}
              aria-current={location.pathname === path ? 'page' : undefined}
            >
              <Icon className="mobile-nav-icon" aria-hidden="true" />
              <span>{label}</span>
            </Link>
          ))}
        </nav>

        <div className="mobile-menu-footer">
          {user ? (
            <>
              <div className="mobile-user-info">
                <div className="mobile-user-avatar" aria-hidden="true">
                  {user.username?.charAt(0).toUpperCase() ?? 'U'}
                </div>
                <div className="mobile-user-details">
                  <div className="mobile-username">{user.username}</div>
                  <div className="mobile-balance">{balance?.toFixed(2) ?? '0.00'} USDC</div>
                </div>
              </div>
              {USER_MENU_ITEMS.map(({ path, label, icon: Icon }) => (
                <Link
                  key={path}
                  to={path}
                  className="mobile-dropdown-item"
                  onClick={() => setIsMobileMenuOpen(false)}
                >
                  <Icon className="mobile-dropdown-icon" aria-hidden="true" />
                  {label}
                </Link>
              ))}
              <button onClick={handleLogout} className="mobile-dropdown-item logout">
                <LogoutIcon className="mobile-dropdown-icon" aria-hidden="true" />
                Log out
              </button>
            </>
          ) : (
            <div className="mobile-auth-buttons">
              <Link to="/login"    className="btn btn-secondary">Log in</Link>
              <Link to="/register" className="btn btn-primary">Get started</Link>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

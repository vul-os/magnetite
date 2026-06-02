import { useState, useRef, useEffect, useCallback } from 'react';
import { Link, useNavigate, useLocation } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { useWallet } from '../hooks/useWallet';
import { usePresence } from '../hooks/usePresence';
import { useTranslation } from '../i18n/useTranslation';
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

// Inline DM icon (not in shared icon set)
function DmIcon(props) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </svg>
  );
}

const NAV_LINKS = [
  { path: '/',             labelKey: 'nav.marketplace', icon: HomeIcon },
  { path: '/communities',  labelKey: 'nav.communities', icon: UsersIcon },
  { path: '/developers',   labelKey: 'nav.developers',  icon: UsersIcon },
  { path: '/leaderboard',  labelKey: 'nav.leaderboard', icon: TrophyIcon },
  { path: '/about',        labelKey: 'nav.about',       icon: UsersIcon },
];

/** Map a presence status to its dot CSS class. */
function presenceDotClass(status) {
  switch (status) {
    case 'online':  return 'presence-dot-online';
    case 'idle':    return 'presence-dot-idle';
    case 'dnd':     return 'presence-dot-dnd';
    case 'offline':
    default:        return 'presence-dot-offline';
  }
}

const USER_MENU_ITEMS = [
  { path: '/profile',  labelKey: 'nav.profile',  icon: UsersIcon },
  { path: '/settings', labelKey: 'nav.settings', icon: SettingsIcon },
  { path: '/earnings', labelKey: 'developer.earnings', icon: WalletIcon },
];

// Sample notifications — in production, these come from the notification API
const SAMPLE_NOTIFICATIONS = [
  { id: 1, type: 'achievement', title: 'Achievement Unlocked', message: 'You earned the "Top Developer" badge', time: '2h ago', unread: true },
  { id: 2, type: 'payout',      title: 'Payout Received',      message: 'You received 50.00 USDC',             time: '5h ago', unread: true },
  { id: 3, type: 'invite',      title: 'Game Invite',           message: 'PlayerX invited you to join a lobby', time: '1d ago', unread: false },
];

export default function Navbar() {
  const { t } = useTranslation();
  const { user, logout } = useAuth();
  const { balance } = useWallet();
  const navigate   = useNavigate();
  const location   = useLocation();

  // Current user presence
  const currentUserId = user?.id ?? null;
  const { getPresence } = usePresence(currentUserId ? [currentUserId] : []);
  const myPresence = currentUserId ? getPresence(currentUserId) : { status: 'offline' };
  const myDotClass = presenceDotClass(myPresence.status);

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

  const handleSearch = (e) => {
    e.preventDefault();
    const q = searchQuery.trim();
    if (q) {
      navigate(`/marketplace?search=${encodeURIComponent(q)}`);
      setSearchQuery('');
      setIsSearchOpen(false);
    }
  };

  const unreadCount = SAMPLE_NOTIFICATIONS.filter(n => n.unread).length;

  return (
    <>
      {/* ── Main bar ───────────────────────────────────────────────────────── */}
      <nav className={`navbar${isScrolled ? ' scrolled' : ''}`} role="navigation" aria-label="Main navigation">
        <div className="navbar-container">

          {/* Left: logo + nav links */}
          <div className="navbar-left">
            <Link to="/home" className="navbar-logo" aria-label={t('navbar.logoLabel')}>
              <div className="logo-icon" aria-hidden="true">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>

            <nav className="navbar-nav" aria-label={t('navbar.siteNav')}>
              {NAV_LINKS.map(({ path, labelKey }) => (
                <Link
                  key={path}
                  to={path}
                  className={`nav-link${location.pathname === path ? ' active' : ''}`}
                  aria-current={location.pathname === path ? 'page' : undefined}
                >
                  {t(labelKey)}
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
                aria-label={t('navbar.openSearch')}
                aria-expanded={isSearchOpen}
              >
                <SearchIcon />
              </button>
              <form onSubmit={handleSearch} className="search-form" role="search">
                <input
                  type="search"
                  placeholder={t('navbar.searchPlaceholder')}
                  value={searchQuery}
                  onChange={e => setSearchQuery(e.target.value)}
                  className="search-input"
                  aria-label={t('navbar.searchLabel')}
                  autoFocus={isSearchOpen}
                />
              </form>
            </div>

            {user ? (
              <>
                {/* Wallet balance */}
                <Link to="/wallet" className="wallet-balance" aria-label={t('navbar.walletLabel', { amount: balance?.toFixed(2) ?? '0.00' })}>
                  <WalletIcon className="wallet-icon" aria-hidden="true" />
                  <span className="balance-amount">{balance?.toFixed(2) ?? '0.00'}</span>
                  <span className="balance-currency">USDC</span>
                </Link>

                {/* Direct Messages */}
                <Link
                  to="/messages"
                  className={`dm-nav-btn${location.pathname === '/messages' ? ' active' : ''}`}
                  aria-label={t('navbar.directMessages')}
                  aria-current={location.pathname === '/messages' ? 'page' : undefined}
                >
                  <DmIcon />
                </Link>

                {/* Notifications */}
                <div ref={notificationsRef} className="notifications-wrapper">
                  <button
                    className="notifications-btn"
                    onClick={() => setIsNotificationsOpen(v => !v)}
                    aria-label={unreadCount > 0 ? t('navbar.notificationsUnread', { count: unreadCount }) : t('navbar.notificationsLabel')}
                    aria-expanded={isNotificationsOpen}
                    aria-haspopup="true"
                  >
                    <BellIcon />
                    {unreadCount > 0 && (
                      <span className="notification-badge" aria-hidden="true">{unreadCount}</span>
                    )}
                  </button>

                  {isNotificationsOpen && (
                    <div className="notification-dropdown" role="dialog" aria-label={t('navbar.notificationsDialog')}>
                      <div className="dropdown-header">
                        <h3>{t('notifications.title')}</h3>
                        {unreadCount > 0 && (
                          <button className="mark-all-read">{t('navbar.markAllRead')}</button>
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
                            <p>{t('navbar.noNotificationsYet')}</p>
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
                    aria-label={t('navbar.userMenuLabel', { username: user.username ?? 'account' })}
                  >
                    <div className="user-avatar-wrapper" aria-hidden="true">
                      <div className="user-avatar">
                        {user.username?.charAt(0).toUpperCase() ?? 'U'}
                      </div>
                      <span className={`navbar-presence-dot ${myDotClass}`} aria-label={t('navbar.statusLabel', { status: myPresence.status })} />
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
                      {USER_MENU_ITEMS.map(({ path, labelKey, icon: Icon }) => (
                        <Link
                          key={path}
                          to={path}
                          className="dropdown-item"
                          role="menuitem"
                          onClick={() => setIsUserMenuOpen(false)}
                        >
                          <Icon className="dropdown-icon" aria-hidden="true" />
                          {t(labelKey)}
                        </Link>
                      ))}
                      <div className="dropdown-divider" role="separator" />
                      <button
                        onClick={handleLogout}
                        className="dropdown-item logout"
                        role="menuitem"
                      >
                        <LogoutIcon className="dropdown-icon" aria-hidden="true" />
                        {t('navbar.logOut')}
                      </button>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="auth-buttons">
                <Link to="/login"    className="btn btn-secondary">{t('navbar.logIn')}</Link>
                <Link to="/register" className="btn btn-primary">{t('navbar.getStarted')}</Link>
              </div>
            )}

            {/* Mobile drawer toggle */}
            <button
              className="mobile-menu-toggle"
              onClick={() => setIsMobileMenuOpen(v => !v)}
              aria-label={isMobileMenuOpen ? t('navbar.closeMenu') : t('navbar.openMenu')}
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
        aria-label={t('navbar.mobileNavDialog')}
      >
        <div className="mobile-menu-header">
          <Link to="/" className="navbar-logo" onClick={() => setIsMobileMenuOpen(false)}>
            <div className="logo-icon" aria-hidden="true">M</div>
            <span className="logo-text">Magnetite</span>
          </Link>
          <button
            className="mobile-menu-close"
            onClick={() => setIsMobileMenuOpen(false)}
            aria-label={t('navbar.closeNav')}
          >
            <CloseIcon />
          </button>
        </div>

        <div className="mobile-search">
          <form onSubmit={handleSearch} role="search">
            <input
              type="search"
              placeholder={t('navbar.searchPlaceholder')}
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              className="search-input"
              aria-label={t('navbar.searchLabel')}
            />
          </form>
        </div>

        <nav className="mobile-nav" aria-label={t('navbar.mobileNav')}>
          {NAV_LINKS.map(({ path, labelKey, icon: Icon }) => (
            <Link
              key={path}
              to={path}
              className={`mobile-nav-link${location.pathname === path ? ' active' : ''}`}
              aria-current={location.pathname === path ? 'page' : undefined}
            >
              <Icon className="mobile-nav-icon" aria-hidden="true" />
              <span>{t(labelKey)}</span>
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
              {USER_MENU_ITEMS.map(({ path, labelKey, icon: Icon }) => (
                <Link
                  key={path}
                  to={path}
                  className="mobile-dropdown-item"
                  onClick={() => setIsMobileMenuOpen(false)}
                >
                  <Icon className="mobile-dropdown-icon" aria-hidden="true" />
                  {t(labelKey)}
                </Link>
              ))}
              <button onClick={handleLogout} className="mobile-dropdown-item logout">
                <LogoutIcon className="mobile-dropdown-icon" aria-hidden="true" />
                {t('navbar.logOut')}
              </button>
            </>
          ) : (
            <div className="mobile-auth-buttons">
              <Link to="/login"    className="btn btn-secondary">{t('navbar.logIn')}</Link>
              <Link to="/register" className="btn btn-primary">{t('navbar.getStarted')}</Link>
            </div>
          )}
        </div>
      </div>
    </>
  );
}

/**
 * BottomNav — mobile bottom navigation bar.
 *
 * Shown only on screens ≤768px (controlled via CSS).
 * Five destinations: Home / Store / Play / Communities / Profile.
 * 44px touch targets, aria-current, safe-area insets.
 */
import { Link, useLocation } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import './BottomNav.css';

/* ── Icons (inline SVG — no dependency on icon bundle) ────────────────────── */

function HomeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
      <polyline points="9 22 9 12 15 12 15 22"/>
    </svg>
  );
}

function StoreIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"/>
      <line x1="3" y1="6" x2="21" y2="6"/>
      <path d="M16 10a4 4 0 0 1-8 0"/>
    </svg>
  );
}

function PlayIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="2" y="6" width="20" height="12" rx="2"/>
      <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01"/>
    </svg>
  );
}

function CommunitiesIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
      <circle cx="9" cy="7" r="4"/>
      <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
      <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
    </svg>
  );
}

function ProfileIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
      <circle cx="12" cy="7" r="4"/>
    </svg>
  );
}

/* ── Tab definitions ──────────────────────────────────────────────────────── */

/**
 * Tab destinations.
 *
 * `/play` and `/profile` used to be listed here and both 404'd: the router only
 * has `/play/:id` (a specific game) and `/profile/:username`. Neither is
 * reachable without an argument, so:
 *   - Play    → `/matchmaking`, which is where you go to get into a game.
 *   - Profile → `/profile/<your username>`, resolved per user at render, and
 *               `/login` when signed out.
 */
const TABS = [
  { path: '/home',        label: 'Home',        Icon: HomeIcon },
  { path: '/marketplace', label: 'Store',       Icon: StoreIcon },
  { path: '/matchmaking', label: 'Play',        Icon: PlayIcon },
  { path: '/communities', label: 'Communities', Icon: CommunitiesIcon },
  { path: '/profile',     label: 'Profile',     Icon: ProfileIcon },
];

/* ─────────────────────────────────────────────────────────────────────────── */

export default function BottomNav() {
  const location = useLocation();
  const { user } = useAuth();

  /* `/profile` alone is not a route. Point at this user's own profile, or at
     sign-in when there is nobody to show. */
  const profileHref = user?.username
    ? `/profile/${encodeURIComponent(user.username)}`
    : '/login';

  const hrefFor = (tabPath) => (tabPath === '/profile' ? profileHref : tabPath);

  /* Resolve the active tab: exact match first, then prefix match */
  function isActive(tabPath) {
    if (location.pathname === tabPath) return true;
    // Special-case: /play/:id and /lobby/:id → highlight Play
    if (tabPath === '/matchmaking' &&
        (location.pathname.startsWith('/play/') || location.pathname.startsWith('/lobby/'))) return true;
    // Special-case: /home or / → highlight Home
    if (tabPath === '/home' && location.pathname === '/') return true;
    // Special-case: /profile/:username → highlight Profile
    if (tabPath === '/profile' && location.pathname.startsWith('/profile/')) return true;
    return false;
  }

  return (
    <nav
      className="bottom-nav"
      aria-label="Main navigation"
      role="navigation"
    >
      {TABS.map(({ path, label, Icon }) => {
        const active = isActive(path);
        return (
          <Link
            key={path}
            to={hrefFor(path)}
            className={`bottom-nav-tab${active ? ' active' : ''}`}
            aria-current={active ? 'page' : undefined}
            aria-label={label}
          >
            <span className="bottom-nav-icon">
              <Icon />
            </span>
            <span className="bottom-nav-label">{label}</span>
          </Link>
        );
      })}
    </nav>
  );
}

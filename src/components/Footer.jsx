import { Link } from 'react-router-dom';
import { GithubIcon, TwitterIcon, DiscordIcon } from '../assets/icons';
import './Footer.css';

const LINK_COLUMNS = [
  {
    title: 'Platform',
    links: [
      { label: 'Marketplace', path: '/marketplace' },
      { label: 'Developers',  path: '/developers' },
      { label: 'Pricing',     path: '/pricing' },
      { label: 'FAQ',         path: '/faq' },
    ],
  },
  {
    title: 'Developers',
    links: [
      { label: 'SDK',           path: '/developers/sdk' },
      { label: 'Documentation', path: '/docs' },
      { label: 'GitHub',        path: 'https://github.com/anomalyco/magnetite', external: true },
      { label: 'Discord',       path: 'https://discord.gg/magnetite',           external: true },
    ],
  },
  {
    title: 'Company',
    links: [
      { label: 'About',    path: '/about' },
      { label: 'Careers',  path: '/careers' },
      { label: 'Contact',  path: '/contact' },
      { label: 'Blog',     path: '/blog' },
    ],
  },
  {
    title: 'Legal',
    links: [
      { label: 'Terms',   path: '/terms' },
      { label: 'Privacy', path: '/privacy' },
      { label: 'Cookies', path: '/cookies' },
    ],
  },
];

const SOCIAL_LINKS = [
  { href: 'https://discord.gg/magnetite',          Icon: DiscordIcon, label: 'Discord' },
  { href: 'https://twitter.com/magnetite',         Icon: TwitterIcon, label: 'Twitter / X' },
  { href: 'https://github.com/anomalyco/magnetite', Icon: GithubIcon,  label: 'GitHub' },
];

// Inline Rust-crab-silhouette (simple ferris-style icon)
function RustIcon({ size = 16 }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="currentColor"
      aria-hidden="true"
      focusable="false"
    >
      <circle cx="16" cy="16" r="6" />
      <rect x="14" y="2"  width="4" height="5" rx="2" />
      <rect x="14" y="25" width="4" height="5" rx="2" />
      <rect x="2"  y="14" width="5" height="4" rx="2" />
      <rect x="25" y="14" width="5" height="4" rx="2" />
      <rect x="5"  y="5"  width="4" height="4" rx="2" transform="rotate(45 7 7)" />
      <rect x="23" y="5"  width="4" height="4" rx="2" transform="rotate(45 25 7)" />
      <rect x="5"  y="23" width="4" height="4" rx="2" transform="rotate(45 7 25)" />
      <rect x="23" y="23" width="4" height="4" rx="2" transform="rotate(45 25 25)" />
    </svg>
  );
}

export default function Footer() {
  const year = new Date().getFullYear();

  return (
    <footer className="footer" role="contentinfo">
      <div className="footer-container">

        {/* ── Top: brand + link columns ───────────────────────────────────── */}
        <div className="footer-top">

          {/* Brand column */}
          <div className="footer-brand">
            <Link to="/" className="footer-logo" aria-label="Magnetite home">
              <div className="logo-icon" aria-hidden="true">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>

            <p className="footer-tagline">
              The open-source platform for building,<br />
              distributing, and monetizing Rust games<br />
              — at any scale.
            </p>

            {/* Rust badge */}
            <div className="footer-rust-badge" aria-label="Built in Rust">
              <div className="rust-lang-icon">
                <RustIcon size={14} />
              </div>
              <span className="kicker-label">Built in Rust</span>
            </div>

            {/* Social */}
            <div className="footer-social" aria-label="Social media links">
              {SOCIAL_LINKS.map(({ href, Icon, label }) => (
                <a
                  key={label}
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="social-link"
                  aria-label={label}
                >
                  <Icon />
                </a>
              ))}
            </div>
          </div>

          {/* Link columns */}
          <div className="footer-links">
            {LINK_COLUMNS.map(column => (
              <div key={column.title} className="footer-column">
                <h3 className="footer-column-title">{column.title}</h3>
                <ul className="footer-column-links">
                  {column.links.map(link => (
                    <li key={link.label}>
                      {link.external || link.path.startsWith('http') ? (
                        <a
                          href={link.path}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="footer-link"
                        >
                          {link.label}
                        </a>
                      ) : (
                        <Link to={link.path} className="footer-link">
                          {link.label}
                        </Link>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        {/* Divider */}
        <div className="footer-divider" role="separator" />

        {/* ── Bottom bar ──────────────────────────────────────────────────── */}
        <div className="footer-bottom">
          <p className="footer-copyright">
            &copy; {year} Magnetite.{' '}
            <Link to="/terms">Terms</Link>{' · '}
            <Link to="/privacy">Privacy</Link>
          </p>

          <div className="footer-bottom-right">
            <span className="footer-open-source">
              <a
                href="https://github.com/anomalyco/magnetite"
                target="_blank"
                rel="noopener noreferrer"
              >
                MIT open source
              </a>
            </span>

            <div className="footer-badge" aria-label="Built with Rust">
              <span className="badge-icon">
                <RustIcon size={13} />
              </span>
              <span>Rust · Bevy · WASM</span>
            </div>
          </div>
        </div>
      </div>
    </footer>
  );
}

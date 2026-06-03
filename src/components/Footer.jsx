import { Link } from 'react-router-dom';
import { GithubIcon, TwitterIcon, DiscordIcon } from '../assets/icons';
import { useTranslation } from '../i18n/useTranslation';
import './Footer.css';

// Column definitions use i18n keys; labels are resolved via t() in the component.
const LINK_COLUMNS = [
  {
    titleKey: 'footer.columns.platform',
    links: [
      { labelKey: 'footer.links.marketplace', path: '/marketplace' },
      { labelKey: 'footer.links.developers',  path: '/developers' },
      { labelKey: 'footer.links.pricing',     path: '/pricing' },
      { labelKey: 'footer.links.faq',         path: '/faq' },
    ],
  },
  {
    titleKey: 'footer.columns.developers',
    links: [
      { labelKey: 'footer.links.sdk',           path: '/developers/sdk' },
      { labelKey: 'footer.links.documentation', path: '/docs' },
      { labelKey: 'footer.links.github',        path: 'https://github.com/anomalyco/magnetite', external: true },
      { labelKey: 'footer.links.discord',       path: 'https://discord.gg/magnetite',           external: true },
    ],
  },
  {
    titleKey: 'footer.columns.company',
    links: [
      { labelKey: 'footer.links.about',    path: '/about' },
      { labelKey: 'footer.links.careers',  path: '/careers' },
      { labelKey: 'footer.links.contact',  path: '/contact' },
      { labelKey: 'footer.links.blog',     path: '/blog' },
    ],
  },
  {
    titleKey: 'footer.columns.legal',
    links: [
      { labelKey: 'footer.links.terms',   path: '/terms' },
      { labelKey: 'footer.links.privacy', path: '/privacy' },
      { labelKey: 'footer.links.cookies', path: '/cookies' },
    ],
  },
];

/** Supported locales for the language selector. */
const SUPPORTED_LOCALES = [
  { code: 'en', labelKey: 'footer.languageSelector.english' },
  { code: 'es', labelKey: 'footer.languageSelector.spanish' },
  { code: 'fr', labelKey: 'footer.languageSelector.french' },
  // Future: { code: 'de', labelKey: 'footer.languageSelector.german' },
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
  const { t, locale, setLocale } = useTranslation();
  const year = new Date().getFullYear();

  return (
    <footer className="footer" role="contentinfo">
      <div className="footer-container">

        {/* ── Top: brand + link columns ───────────────────────────────────── */}
        <div className="footer-top">

          {/* Brand column */}
          <div className="footer-brand">
            <Link to="/" className="footer-logo" aria-label={t('footer.logoLabel')}>
              <div className="logo-icon" aria-hidden="true">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>

            <p className="footer-tagline">{t('footer.tagline')}</p>

            {/* Rust badge */}
            <div className="footer-rust-badge" aria-label={t('footer.builtInRust')}>
              <div className="rust-lang-icon">
                <RustIcon size={14} />
              </div>
              <span className="kicker-label">{t('footer.builtInRust')}</span>
            </div>

            {/* Social */}
            <div className="footer-social" aria-label={t('footer.socialLinks')}>
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

            {/* ── Language selector ─────────────────────────────────────── */}
            <div className="footer-lang-selector">
              <label htmlFor="footer-lang-select" className="footer-lang-label">
                {t('footer.languageSelector.label')}
              </label>
              <select
                id="footer-lang-select"
                className="footer-lang-select"
                value={locale}
                onChange={e => setLocale(e.target.value)}
                aria-label={t('footer.languageSelector.label')}
              >
                {SUPPORTED_LOCALES.map(({ code, labelKey }) => (
                  <option key={code} value={code}>
                    {t(labelKey)}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Link columns */}
          <div className="footer-links">
            {LINK_COLUMNS.map(column => (
              <div key={column.titleKey} className="footer-column">
                <h3 className="footer-column-title">{t(column.titleKey)}</h3>
                <ul className="footer-column-links">
                  {column.links.map(link => (
                    <li key={link.labelKey}>
                      {link.external || link.path.startsWith('http') ? (
                        <a
                          href={link.path}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="footer-link"
                        >
                          {t(link.labelKey)}
                        </a>
                      ) : (
                        <Link to={link.path} className="footer-link">
                          {t(link.labelKey)}
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
            {t('footer.copyright', { year })}{' '}
            <Link to="/terms">{t('footer.terms')}</Link>{' · '}
            <Link to="/privacy">{t('footer.privacy')}</Link>
          </p>

          <div className="footer-bottom-right">
            <span className="footer-open-source">
              <a
                href="https://github.com/anomalyco/magnetite"
                target="_blank"
                rel="noopener noreferrer"
              >
                {t('footer.openSource')}
              </a>
            </span>

            <div className="footer-badge" aria-label={t('footer.builtInRust')}>
              <span className="badge-icon">
                <RustIcon size={13} />
              </span>
              <span>{t('footer.techStack')}</span>
            </div>
          </div>
        </div>
      </div>
    </footer>
  );
}

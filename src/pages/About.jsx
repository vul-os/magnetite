import './About.css';
import { useTranslation } from '../i18n/useTranslation';
import magnetiteLogo from '../assets/magnetite-logo.svg';

const timeline = [
  { year: '2024 Q1',   event: 'Magnetite founded — vision: open-source Rust gaming at any scale.' },
  { year: '2024 Q3',   event: 'First Bevy game compiled to WASM and deployed on the platform.' },
  { year: '2025 Q1',   event: 'Non-custodial USDC checkout launched — buyers pay developer wallets directly, entitlements backed by signed receipts.' },
  { year: '2026 Q1',   event: 'Real-time multiplayer matchmaking & server-authoritative netcode released.' },
];

export default function About() {
  const { t } = useTranslation();

  return (
    <div className="about-page">
      {/* ── Hero ───────────────────────────────────────────────────────────── */}
      <section className="about-hero bg-atmosphere" aria-labelledby="about-heading">
        <div className="magnetic-field" aria-hidden="true">
          <div className="field-line field-line-1" />
          <div className="field-line field-line-2" />
          <div className="field-line field-line-3" />
          <div className="field-line field-line-4" />
          <div className="field-line field-line-5" />
        </div>
        <div className="hero-content reveal">
          <span className="kicker reveal-1">{t('about.kicker')}</span>
          <h1 id="about-heading" className="hero-title reveal-2">
            {t('about.heroTitle')}<br />
            <span className="gradient-text">{t('about.heroTitleHighlight')}</span>
          </h1>
          <p className="hero-subtitle reveal-3">{t('about.heroSubtitle')}</p>
        </div>
      </section>

      {/* ── Mission ────────────────────────────────────────────────────────── */}
      <section className="mission-section">
        <div className="container">
          <div className="mission-content">
            <span className="kicker">{t('about.missionKicker')}</span>
            <h2>{t('about.missionHeading')}</h2>
            <p>{t('about.missionP1')}</p>
            <p>{t('about.missionP2')}</p>
          </div>
        </div>
      </section>

      {/* ── Timeline ───────────────────────────────────────────────────────── */}
      <section className="timeline-section">
        <div className="container">
          <span className="kicker">{t('about.historyKicker')}</span>
          <h2 className="section-title">{t('about.historyHeading')}</h2>
          <ol className="timeline" aria-label={t('about.historyLabel')}>
            {timeline.map((item, i) => (
              <li className="timeline-item" key={i}>
                <span className="timeline-year">{item.year}</span>
                <div className="timeline-dot" aria-hidden="true" />
                <div className="timeline-content">
                  <p>{item.event}</p>
                </div>
              </li>
            ))}
          </ol>
        </div>
      </section>

      {/* ── Open Source ────────────────────────────────────────────────────── */}
      <section className="opensource-section">
        <div className="container">
          <div className="opensource-content">
            <span className="kicker">{t('about.openSourceKicker')}</span>
            <h2>{t('about.openSourceHeading')}</h2>
            <p>{t('about.openSourceBody')}</p>
            <a
              href="https://github.com"
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-primary btn-lg github-link"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
              </svg>
              {t('about.viewOnGitHub')}
            </a>
          </div>
        </div>
      </section>

      {/* ── Press ──────────────────────────────────────────────────────────── */}
      <section className="press-section">
        <div className="container">
          <div className="press-content">
            <span className="kicker">{t('about.pressKicker')}</span>
            <h2>{t('about.pressHeading')}</h2>
            <p>{t('about.pressBody')}</p>
            <a href="/press-kit" className="btn btn-secondary btn-lg">
              {t('about.downloadPressKit')}
            </a>
          </div>
        </div>
      </section>

      {/* ── Contact ────────────────────────────────────────────────────────── */}
      <section className="contact-section">
        <div className="container">
          <span className="kicker">{t('about.contactKicker')}</span>
          <h2 className="section-title">{t('about.contactHeading')}</h2>
          <p className="section-subtitle">{t('about.contactSubtitle')}</p>
          <div className="contact-grid">
            <div className="contact-info">
              <div className="contact-item">
                <div className="contact-icon-wrap" aria-hidden="true">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="2" y="4" width="20" height="16" rx="2" />
                    <path d="M22 7l-10 6L2 7" />
                  </svg>
                </div>
                <div>
                  <h4>{t('about.contactEmail')}</h4>
                  <a href={`mailto:${t('about.contactEmailAddress')}`}>{t('about.contactEmailAddress')}</a>
                </div>
              </div>
              <div className="contact-item">
                <div className="contact-icon-wrap" aria-hidden="true">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
                  </svg>
                </div>
                <div>
                  <h4>{t('about.contactDiscord')}</h4>
                  <a href="#" rel="noopener noreferrer">{t('about.contactDiscordCta')}</a>
                </div>
              </div>
              <div className="contact-item">
                <div className="contact-icon-wrap" aria-hidden="true">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
                  </svg>
                </div>
                <div>
                  <h4>{t('about.contactTwitter')}</h4>
                  <a href="#" rel="noopener noreferrer">{t('about.contactTwitterHandle')}</a>
                </div>
              </div>
            </div>

            <div className="careers-preview">
              <span className="kicker">{t('about.joinUsKicker')}</span>
              <h4>{t('about.joinUsHeading')}</h4>
              <p>{t('about.joinUsBody')}</p>
              <a href="/careers" className="btn btn-primary">{t('about.viewCareers')}</a>
            </div>
          </div>
        </div>
      </section>

      {/* ── Footer ─────────────────────────────────────────────────────────── */}
      <footer className="about-footer">
        <div className="container">
          <div className="footer-content">
            <div className="footer-brand">
              <div className="logo">
                <img src={magnetiteLogo} className="logo-icon" aria-hidden="true" alt="" />
                <span>Magnetite</span>
              </div>
              <p>{t('about.footerTagline')}</p>
            </div>
            <nav className="footer-links" aria-label={t('about.footerNav')}>
              <a href="/marketplace">{t('footer.links.marketplace')}</a>
              <a href="/about">{t('nav.about')}</a>
              <a href="/careers">{t('footer.links.careers')}</a>
              <a href="https://github.com" target="_blank" rel="noopener noreferrer">{t('footer.links.github')}</a>
            </nav>
          </div>
          <div className="footer-bottom">
            <p>{t('about.footerCopyright')}</p>
          </div>
        </div>
      </footer>
    </div>
  );
}

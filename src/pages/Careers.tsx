import './Careers.css';
import magnetiteLogo from '../assets/magnetite-logo.svg';

const openPositions = [
  {
    title: 'Platform Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'Rust',
    description:
      'Build and maintain our core platform — distributed systems, game-server netcode, payment processing, and real-time matchmaking. Rust required.',
  },
  {
    title: 'Game Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'Bevy / Rust',
    description:
      'Create compelling multiplayer game experiences using Bevy ECS. Collaborate with designers to bring concepts from prototype to production-quality Rust.',
  },
  {
    title: 'Frontend Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'React',
    description:
      'Build high-craft, performant interfaces for our gaming platform — player dashboards, game storefronts, and developer tooling.',
  },
  {
    title: 'Security Engineer',
    department: 'Security',
    location: 'Remote',
    tech: 'Security',
    description:
      'Own platform security end-to-end: audits, sandbox hardening, WASM security model, payment security, and protecting player data and assets.',
  },
];

const benefits = [
  {
    kicker: '// REMOTE',
    title: 'Remote-First',
    desc: 'Work from anywhere in the world. No office required, asynchronous by default.',
  },
  {
    kicker: '// PAY',
    title: 'Competitive Pay',
    desc: 'Market-leading salaries paid in your local currency via Wise.',
  },
  {
    kicker: '// CULTURE',
    title: 'Rust & Gaming',
    desc: 'Play and build Rust games as part of your job. You ship what you enjoy.',
  },
  {
    kicker: '// IMPACT',
    title: 'Shape an Industry',
    desc: 'Join early and define how Rust games are distributed and monetized at scale.',
  },
];

export default function Careers() {
  return (
    <div className="careers-page">
      {/* ── Hero ───────────────────────────────────────────────────────────── */}
      <section className="careers-hero bg-atmosphere" aria-labelledby="careers-heading">
        <div className="magnetic-field" aria-hidden="true">
          <div className="field-line field-line-1" />
          <div className="field-line field-line-2" />
          <div className="field-line field-line-3" />
          <div className="field-line field-line-4" />
          <div className="field-line field-line-5" />
        </div>
        <div className="hero-content reveal">
          <div className="hiring-badge reveal-1" role="status">
            <span className="pulse" aria-hidden="true" />
            We&apos;re Hiring
          </div>
          <span className="kicker reveal-1">// OPEN POSITIONS</span>
          <h1 id="careers-heading" className="hero-title reveal-2">
            Join the <span className="gradient-text">Magnetite</span> Team
          </h1>
          <p className="hero-subtitle reveal-3">
            Help us build the platform for Rust games at any scale.
            We&apos;re looking for engineers who care deeply about developer experience and open source.
            We&apos;re remote-first and asynchronous by default.
          </p>
        </div>
      </section>

      {/* ── Benefits ───────────────────────────────────────────────────────── */}
      <section className="benefits-section">
        <div className="container">
          <span className="kicker">// PERKS</span>
          <h2 className="section-title">Why work with us</h2>
          <p className="section-subtitle">We take care of the people who build Magnetite</p>
          <div className="benefits-grid">
            {benefits.map((b, i) => (
              <div className="benefit-card" key={i}>
                <span className="benefit-kicker">{b.kicker}</span>
                <h3>{b.title}</h3>
                <p>{b.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Positions ──────────────────────────────────────────────────────── */}
      <section className="positions-section">
        <div className="container">
          <span className="kicker">// JOIN THE TEAM</span>
          <h2 className="section-title">Open positions</h2>
          <p className="section-subtitle">Find your role in building the future of Rust gaming</p>
          <div className="positions-list">
            {openPositions.map((position, i) => (
              <div className="position-card" key={i}>
                <div className="position-header">
                  <div className="position-title-row">
                    <h3>{position.title}</h3>
                    <span className="tech-badge">{position.tech}</span>
                  </div>
                  <div className="position-meta">
                    <span className="meta-item">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                        <circle cx="12" cy="10" r="3" />
                        <path d="M12 2a8 8 0 0 1 8 8c0 5.4-7 12-8 12S4 15.4 4 10a8 8 0 0 1 8-8z" />
                      </svg>
                      {position.location}
                    </span>
                    <span className="meta-item">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                        <rect x="2" y="7" width="20" height="14" rx="2" />
                        <path d="M16 7V5a2 2 0 0 0-4 0v2M8 7V5a2 2 0 0 0-4 0v2" />
                      </svg>
                      {position.department}
                    </span>
                  </div>
                </div>
                <p className="position-description">{position.description}</p>
                <a
                  href={`mailto:careers@magnetite.gg?subject=Application: ${position.title}`}
                  className="btn btn-primary apply-btn"
                >
                  Apply Now
                </a>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Open CTA ───────────────────────────────────────────────────────── */}
      <section className="open-source-section">
        <div className="container">
          <div className="opensource-content">
            <span className="kicker">// DON&apos;T SEE A FIT?</span>
            <h2>Send an open application</h2>
            <p>
              If you&apos;re passionate about Rust, gaming, or building developer tools that empower
              indie studios, we&apos;d still love to hear from you.
            </p>
            <a
              href="mailto:careers@magnetite.gg?subject=General Application"
              className="btn btn-secondary btn-lg"
            >
              Send us your resume
            </a>
          </div>
        </div>
      </section>

      {/* ── Footer ─────────────────────────────────────────────────────────── */}
      <footer className="careers-footer">
        <div className="container">
          <div className="footer-content">
            <div className="footer-brand">
              <div className="logo">
                <img src={magnetiteLogo} className="logo-icon" aria-hidden="true" alt="" />
                <span>Magnetite</span>
              </div>
              <p>Open-source Rust gaming at any scale.</p>
            </div>
            <nav className="footer-links" aria-label="Footer navigation">
              <a href="/marketplace">Marketplace</a>
              <a href="/about">About</a>
              <a href="/contact">Contact</a>
              <a href="https://github.com" target="_blank" rel="noopener noreferrer">GitHub</a>
            </nav>
          </div>
          <div className="footer-bottom">
            <p>© 2026 Magnetite. Open source under MIT License.</p>
          </div>
        </div>
      </footer>
    </div>
  );
}

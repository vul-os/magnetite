import './Home.css';

const features = [
  {
    kicker: '// RUST-NATIVE',
    title: 'Author in Rust, Ship Everywhere',
    description: 'Write Bevy game logic once. Magnetite compiles to WASM for browsers and native for desktop — transparent code, fair by design.',
  },
  {
    kicker: '// USDC PAYMENTS',
    title: 'Pay with USDC',
    description: 'No middlemen, instant on-chain settlements. Your money flows directly to developers with a 15% platform fee — everything else is yours.',
  },
  {
    kicker: '// 85% REVENUE',
    title: 'Earn as Developer',
    description: '85% revenue share, weekly USDC payouts. Own your code, own your players, scale from a game-jam entry to a AAA title on the same platform.',
  },
];

const steps = [
  {
    num: '01',
    kicker: '// DISCOVER',
    title: 'Find a Rust game you love',
    desc: 'Browse our catalog of open-source Rust games — from arcade puzzles to real-time multiplayer.',
  },
  {
    num: '02',
    kicker: '// PAY',
    title: 'Connect your USDC wallet',
    desc: 'Scan or paste your wallet address. Paystack fiat on-ramp available for ZAR and other currencies.',
  },
  {
    num: '03',
    kicker: '// PLAY',
    title: 'Play and climb the ranks',
    desc: 'Win matches, unlock achievements, and climb platform-wide leaderboards. Withdraw earnings anytime.',
  },
];

const devFeatures = [
  {
    kicker: '// OWNERSHIP',
    title: 'Full Code Ownership',
    desc: 'MIT license on SDK and platform. Your game, your rules — forever.',
  },
  {
    kicker: '// DX',
    title: 'One-Command Deploy',
    desc: 'cargo magnetite deploy. GitHub integration auto-builds WASM and native on push.',
  },
  {
    kicker: '// SCALE',
    title: 'Game-Jam to AAA',
    desc: 'The same SDK and API for a single-file arcade game and a 64-player MMO.',
  },
];

const footerLinks = {
  platform: [
    { label: 'Marketplace', href: '/marketplace' },
    { label: 'Wallet', href: '/wallet' },
    { label: 'Leaderboards', href: '/leaderboards' },
    { label: 'Pricing', href: '/pricing' },
  ],
  developers: [
    { label: 'Documentation', href: '/docs' },
    { label: 'Developer Studio', href: '/developers/studio' },
    { label: 'SDK Reference', href: '/docs/sdk' },
    { label: 'API Access', href: '/docs/api' },
  ],
  company: [
    { label: 'About', href: '/about' },
    { label: 'Careers', href: '/careers' },
    { label: 'Contact', href: '/contact' },
    { label: 'Blog', href: '/blog' },
  ],
};

export default function Home() {
  return (
    <div className="home">
      {/* ── HERO ─────────────────────────────────────────────────────────── */}
      <section className="hero" aria-labelledby="home-hero-heading">
        <div className="magnetic-field" aria-hidden="true">
          <div className="field-line field-line-1" />
          <div className="field-line field-line-2" />
          <div className="field-line field-line-3" />
          <div className="field-line field-line-4" />
          <div className="field-line field-line-5" />
          <div className="field-arc arc-1" />
          <div className="field-arc arc-2" />
          <div className="field-arc arc-3" />
          <div className="field-arc arc-4" />
          <div className="field-particle" style={{ top: '20%', left: '15%', animationDuration: '12s' }} />
          <div className="field-particle" style={{ top: '40%', left: '80%', animationDuration: '10s', animationDelay: '2s' }} />
          <div className="field-particle" style={{ top: '70%', left: '25%', animationDuration: '14s', animationDelay: '4s' }} />
          <div className="field-particle" style={{ top: '60%', left: '70%', animationDuration: '11s', animationDelay: '1s' }} />
        </div>

        <div className="hero-content">
          <span className="kicker">// OPEN SOURCE · RUST-NATIVE · MONETIZED</span>
          <div className="hero-badge">
            <span className="hero-badge-dot" aria-hidden="true" />
            Now live — USDC payments &amp; real-time multiplayer
          </div>
          <h1 id="home-hero-heading" className="hero-title">
            <span className="line">Rust Games.</span>
            <span className="line">Real Money.</span>
            <span className="line gradient-text">At Any Scale.</span>
          </h1>
          <p className="hero-subtitle">
            The open-source platform where developers publish Rust games,
            players keep their winnings, and no middlemen take a cut.
          </p>
          <div className="hero-ctas">
            <a href="/marketplace" className="btn btn-primary btn-lg">Start Playing</a>
            <a href="/developers/studio" className="btn btn-secondary btn-lg">Host Your Game</a>
          </div>
        </div>

        <div className="hero-visual" aria-hidden="true">
          <div className="controller-container">
            <div className="floating-particle particle-1" />
            <div className="floating-particle particle-2" />
            <div className="floating-particle particle-3" />
            <div className="floating-particle particle-4" />
            <div className="floating-particle particle-5" />
            <div className="controller-float">
              <div className="magnet-ring ring-1" />
              <div className="magnet-ring ring-2" />
              <div className="magnet-ring ring-3" />
              <div className="controller-icon" role="img" aria-label="Rust crab mascot">
                🦀
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── SOCIAL PROOF ─────────────────────────────────────────────────── */}
      <section className="social-proof" aria-label="Platform statistics">
        <div className="container">
          <div className="social-proof-grid">
            <div className="stat-item">
              <span className="stat-number">2,847</span>
              <span className="stat-label">Developers</span>
            </div>
            <div className="stat-divider" aria-hidden="true" />
            <div className="stat-item">
              <span className="stat-number">156+</span>
              <span className="stat-label">Games Hosted</span>
            </div>
            <div className="stat-divider" aria-hidden="true" />
            <div className="stat-item">
              <span className="stat-number">$2.4M</span>
              <span className="stat-label">USDC Paid Out</span>
            </div>
          </div>
        </div>
      </section>

      {/* ── FEATURES ─────────────────────────────────────────────────────── */}
      <section className="features" aria-labelledby="home-features-heading">
        <div className="container">
          <div className="section-header">
            <span className="section-label">// WHY MAGNETITE</span>
            <h2 id="home-features-heading" className="section-title">
              Gaming without compromise
            </h2>
            <p className="section-subtitle">
              A new paradigm where fairness is guaranteed by transparency,
              not trust in centralized platforms.
            </p>
          </div>
          <div className="features-grid">
            {features.map((f, i) => (
              <div className="feature-card" key={i}>
                <span className="feature-kicker">{f.kicker}</span>
                <h3>{f.title}</h3>
                <p>{f.description}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── HOW IT WORKS ─────────────────────────────────────────────────── */}
      <section className="how-it-works" aria-labelledby="home-how-heading">
        <div className="container">
          <div className="section-header">
            <span className="section-label">// HOW IT WORKS</span>
            <h2 id="home-how-heading" className="section-title">
              Start playing in minutes
            </h2>
            <p className="section-subtitle">
              Three simple steps to access a new world of transparent,
              decentralized Rust gaming.
            </p>
          </div>
          <div className="steps-container">
            {steps.map((s, i) => (
              <div className="step-card" key={i}>
                <div className="step-number">{s.num}</div>
                <span className="step-kicker">{s.kicker}</span>
                <h3>{s.title}</h3>
                <p>{s.desc}</p>
                <div className="step-connector" aria-hidden="true" />
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── DEVELOPERS ───────────────────────────────────────────────────── */}
      <section className="developers-section" aria-labelledby="home-dev-heading">
        <div className="container">
          <div className="developers-grid">
            <div className="developers-content">
              <span className="section-label">// FOR RUST DEVELOPERS</span>
              <h2 id="home-dev-heading" className="section-title">
                Built by developers,<br />for developers
              </h2>
              <p className="section-subtitle">
                Stop giving 30% to platforms. Keep your code, own your players,
                and earn what you deserve.
              </p>
              <div className="dev-features">
                {devFeatures.map((f, i) => (
                  <div className="dev-feature-item" key={i}>
                    <div className="dev-feature-kicker">{f.kicker}</div>
                    <div className="dev-feature-content">
                      <h4>{f.title}</h4>
                      <p>{f.desc}</p>
                    </div>
                  </div>
                ))}
              </div>
              <a href="/developers/studio" className="btn btn-primary btn-lg">
                Start Hosting
                <svg
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                  style={{ marginLeft: 6 }}
                >
                  <path d="M5 12h14M12 5l7 7-7 7" />
                </svg>
              </a>
            </div>

            <div className="developers-visual">
              <div className="code-window">
                <div className="code-header">
                  <div className="code-dot red" />
                  <div className="code-dot yellow" />
                  <div className="code-dot green" />
                  <span className="code-title">magnetite.toml</span>
                </div>
                <div className="code-body">
                  <div className="code-line">
                    <span className="line-number">1</span>
                    <span className="code-text">
                      <span className="code-bracket">[game]</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">2</span>
                    <span className="code-text">
                      <span className="code-key">name</span>
                      {' = '}
                      <span className="code-string">&quot;my-rust-game&quot;</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">3</span>
                    <span className="code-text">
                      <span className="code-key">entry</span>
                      {' = '}
                      <span className="code-string">&quot;src/main.rs&quot;</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">4</span>
                    <span className="code-text" />
                  </div>
                  <div className="code-line">
                    <span className="line-number">5</span>
                    <span className="code-text">
                      <span className="code-bracket">[pricing]</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">6</span>
                    <span className="code-text">
                      <span className="code-key">per_session</span>
                      {' = '}
                      <span className="code-number">0.50</span>
                      {'  '}
                      <span className="code-comment"># USDC</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">7</span>
                    <span className="code-text">
                      <span className="code-key">revenue_share</span>
                      {' = '}
                      <span className="code-number">85</span>
                      {'  '}
                      <span className="code-comment"># %</span>
                    </span>
                  </div>
                  <div className="code-line">
                    <span className="line-number">8</span>
                    <span className="code-text" />
                  </div>
                  <div className="code-line">
                    <span className="line-number">9</span>
                    <span className="code-text">
                      <span className="code-comment"># cargo magnetite deploy</span>
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── FOOTER CTA ───────────────────────────────────────────────────── */}
      <section className="footer-cta" aria-labelledby="home-cta-heading">
        <div className="container">
          <div className="footer-cta-content">
            <span className="section-label">// GET STARTED</span>
            <h2 id="home-cta-heading" className="section-title">
              Ready to change<br />how gaming works?
            </h2>
            <p className="section-subtitle">
              Join thousands of developers and players building
              the future of transparent, decentralized Rust gaming.
            </p>
            <div className="cta-buttons">
              <a href="/marketplace" className="btn btn-primary btn-lg">Join the Platform</a>
              <a href="/docs" className="btn btn-secondary btn-lg">Read the Docs</a>
            </div>
          </div>
        </div>
      </section>

      {/* ── FOOTER ───────────────────────────────────────────────────────── */}
      <footer className="home-footer">
        <div className="container">
          <div className="footer-main">
            <div className="footer-brand">
              <div className="footer-logo">
                <div className="footer-logo-icon" aria-hidden="true">M</div>
                <span>Magnetite</span>
              </div>
              <p>Open-source Rust gaming. No middlemen. MIT forever.</p>
            </div>
            <div className="footer-links-grid">
              <div className="footer-link-group">
                <h4>Platform</h4>
                {footerLinks.platform.map((link, i) => (
                  <a key={i} href={link.href}>{link.label}</a>
                ))}
              </div>
              <div className="footer-link-group">
                <h4>Developers</h4>
                {footerLinks.developers.map((link, i) => (
                  <a key={i} href={link.href}>{link.label}</a>
                ))}
              </div>
              <div className="footer-link-group">
                <h4>Company</h4>
                {footerLinks.company.map((link, i) => (
                  <a key={i} href={link.href}>{link.label}</a>
                ))}
              </div>
            </div>
          </div>
          <div className="footer-bottom">
            <p className="footer-copy">
              © 2026 Magnetite. Open source under MIT License.
            </p>
            <div className="footer-social">
              <a href="https://github.com" target="_blank" rel="noopener noreferrer" aria-label="GitHub">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
                </svg>
              </a>
              <a href="https://twitter.com" target="_blank" rel="noopener noreferrer" aria-label="Twitter / X">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
                </svg>
              </a>
              <a href="https://discord.com" target="_blank" rel="noopener noreferrer" aria-label="Discord">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
                </svg>
              </a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}

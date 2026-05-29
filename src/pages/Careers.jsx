import './Careers.css';

const openPositions = [
  {
    title: 'Platform Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'Rust',
    description: 'Build and maintain our core platform infrastructure. Work on distributed systems, payment processing, and real-time matchmaking.'
  },
  {
    title: 'Game Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'Bevy',
    description: 'Create amazing multiplayer game experiences using Bevy ECS. Collaborate with game designers to bring concepts to life.'
  },
  {
    title: 'Frontend Engineer',
    department: 'Engineering',
    location: 'Remote',
    tech: 'React',
    description: 'Build beautiful, performant user interfaces for our gaming platform. Work on player dashboards, game storefronts, and developer tools.'
  },
  {
    title: 'Security Engineer',
    department: 'Security',
    location: 'Remote',
    tech: 'Security',
    description: 'Ensure platform security end-to-end. Conduct audits, implement security best practices, and protect player assets and data.'
  },
];

export default function Careers() {
  return (
    <div className="careers-page">
      <section className="careers-hero">
        <div className="magnetic-field">
          <div className="field-line field-line-1"></div>
          <div className="field-line field-line-2"></div>
          <div className="field-line field-line-3"></div>
          <div className="field-line field-line-4"></div>
          <div className="field-line field-line-5"></div>
        </div>
        <div className="hero-content">
          <div className="hiring-badge">
            <span className="pulse"></span>
            We're Hiring
          </div>
          <h1 className="hero-title">
            Join the <span className="gradient-text">Magnetite</span> Team
          </h1>
          <p className="hero-subtitle">
            Help us build the future of decentralized gaming. We're looking for passionate
            gamers and engineers who want to reshape how games are played and monetized.
          </p>
          <div className="hero-stats">
            <div className="stat">
              <span className="stat-value">12+</span>
              <span className="stat-label">Team Members</span>
            </div>
            <div className="stat">
              <span className="stat-value">12</span>
              <span className="stat-label">Countries</span>
            </div>
            <div className="stat">
              <span className="stat-value">100%</span>
              <span className="stat-label">Remote</span>
            </div>
          </div>
        </div>
      </section>

      <section className="benefits-section">
        <div className="container">
          <h2 className="section-title">Why Work With Us</h2>
          <p className="section-subtitle">Great perks for a great team</p>
          <div className="benefits-grid">
            <div className="benefit-card">
              <div className="benefit-icon">🌍</div>
              <h3>Remote-First</h3>
              <p>Work from anywhere in the world. No office required.</p>
            </div>
            <div className="benefit-card">
              <div className="benefit-icon">💰</div>
              <h3>Competitive Pay</h3>
              <p>Market-leading salaries with USDC payments.</p>
            </div>
            <div className="benefit-card">
              <div className="benefit-icon">🎮</div>
              <h3>Gaming Culture</h3>
              <p>Play games at work. It's literally your job.</p>
            </div>
            <div className="benefit-card">
              <div className="benefit-icon">📈</div>
              <h3>Growth</h3>
              <p>Shape the future of an emerging industry.</p>
            </div>
          </div>
        </div>
      </section>

      <section className="positions-section">
        <div className="container">
          <h2 className="section-title">Open Positions</h2>
          <p className="section-subtitle">Find your role in building the future of gaming</p>
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
                      <span className="meta-icon">📍</span>
                      {position.location}
                    </span>
                    <span className="meta-item">
                      <span className="meta-icon">🏢</span>
                      {position.department}
                    </span>
                  </div>
                </div>
                <p className="position-description">{position.description}</p>
                <a
                  href="mailto:careers@magnetite.gg?subject=Application: {position.title}"
                  className="btn btn-primary apply-btn"
                >
                  Apply Now
                </a>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="open-source-section">
        <div className="container">
          <div className="opensource-content">
            <h2>Don't see a fit?</h2>
            <p>
              We're always looking for talented people. If you're passionate about gaming,
              decentralization, or building tools that empower developers, we'd love to hear from you.
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

      <footer className="careers-footer">
        <div className="container">
          <div className="footer-content">
            <div className="footer-brand">
              <div className="logo">
                <div className="logo-icon">M</div>
                <span>Magnetite</span>
              </div>
              <p>Decentralized gaming. No middlemen.</p>
            </div>
            <div className="footer-links">
              <a href="/marketplace">Marketplace</a>
              <a href="/about">About</a>
              <a href="/contact">Contact</a>
              <a href="https://github.com" target="_blank" rel="noopener noreferrer">GitHub</a>
            </div>
          </div>
          <div className="footer-bottom">
            <p>© 2026 Magnetite. Open source under MIT License.</p>
          </div>
        </div>
      </footer>
    </div>
  );
}

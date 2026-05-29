import './About.css';

const team = [
  { name: 'Alex Chen', role: 'Founder & CEO', avatar: '👨‍💻' },
  { name: 'Sarah Kim', role: 'CTO', avatar: '👩‍💻' },
  { name: 'Marcus Johnson', role: 'Lead Engineer', avatar: '👨‍🔧' },
  { name: 'Emily Rivera', role: 'Game Designer', avatar: '👩‍🎨' },
];

const timeline = [
  { year: '2024', event: 'Magnetite founded with a vision for decentralized gaming' },
  { year: '2024 Q3', event: 'First open source game deployed on the platform' },
  { year: '2025 Q1', event: 'USDC payment integration launched' },
  { year: '2025 Q3', event: '100+ games on the marketplace' },
  { year: '2026', event: 'Real-time multiplayer matchmaking released' },
];

export default function About() {
  return (
    <div className="about-page">
      <section className="about-hero">
        <div className="magnetic-field">
          <div className="field-line field-line-1"></div>
          <div className="field-line field-line-2"></div>
          <div className="field-line field-line-3"></div>
          <div className="field-line field-line-4"></div>
          <div className="field-line field-line-5"></div>
        </div>
        <div className="hero-content">
          <h1 className="hero-title">
            Built for gamers,<br />
            <span className="gradient-text">by gamers</span>
          </h1>
          <p className="hero-subtitle">
            We believe gaming should be fair, transparent, and owned by the community.
            Magnetite is the decentralized platform where players and developers come first.
          </p>
        </div>
      </section>

      <section className="mission-section">
        <div className="container">
          <div className="mission-content">
            <h2>Our Mission</h2>
            <p>
              Magnetite is building the future of gaming—a world where developers own their creations,
              players keep their winnings, and no middlemen take a cut. We leverage blockchain
              technology to create a transparent, trustless gaming ecosystem where everyone plays
              on a level field.
            </p>
            <p>
              Our platform empowers independent developers with 90% revenue shares, instant USDC
              payments, and full ownership of their games. For players, we offer a curated library
              of open source games, provably fair matchmaking, and the ability to truly own in-game assets.
            </p>
          </div>
        </div>
      </section>

      <section className="team-section">
        <div className="container">
          <h2 className="section-title">Meet the Team</h2>
          <p className="section-subtitle">The gamers and engineers building Magnetite</p>
          <div className="team-grid">
            {team.map((member, i) => (
              <div className="team-card" key={i}>
                <div className="team-avatar">{member.avatar}</div>
                <h3>{member.name}</h3>
                <p>{member.role}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="timeline-section">
        <div className="container">
          <h2 className="section-title">Our Journey</h2>
          <div className="timeline">
            {timeline.map((item, i) => (
              <div className="timeline-item" key={i}>
                <div className="timeline-year">{item.year}</div>
                <div className="timeline-dot"></div>
                <div className="timeline-content">
                  <p>{item.event}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="opensource-section">
        <div className="container">
          <div className="opensource-content">
            <h2>Open Source Commitment</h2>
            <p>
              Transparency is at our core. All Magnetite platform code is open source and
              available on GitHub. We believe in community-driven development where anyone
              can audit, contribute, and improve the platform.
            </p>
            <a
              href="https://github.com"
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-primary btn-lg github-link"
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"/>
              </svg>
              View on GitHub
            </a>
          </div>
        </div>
      </section>

      <section className="press-section">
        <div className="container">
          <div className="press-content">
            <h2>Press & Media</h2>
            <p>
              Looking for press materials, logos, or brand assets? Download our complete
              press kit with high-resolution images, company facts, and media resources.
            </p>
            <a href="/press-kit" className="btn btn-secondary btn-lg">
              Download Press Kit
            </a>
          </div>
        </div>
      </section>

      <section className="contact-section">
        <div className="container">
          <h2 className="section-title">Get in Touch</h2>
          <p className="section-subtitle">We'd love to hear from you</p>
          <div className="contact-grid">
            <div className="contact-info">
              <div className="contact-item">
                <span className="contact-icon">📧</span>
                <div>
                  <h4>Email</h4>
                  <a href="mailto:hello@magnetite.gg">hello@magnetite.gg</a>
                </div>
              </div>
              <div className="contact-item">
                <span className="contact-icon">💬</span>
                <div>
                  <h4>Discord</h4>
                  <a href="#">Join our community</a>
                </div>
              </div>
              <div className="contact-item">
                <span className="contact-icon">🐦</span>
                <div>
                  <h4>Twitter</h4>
                  <a href="#">@MagnetiteGG</a>
                </div>
              </div>
            </div>
            <div className="careers-preview">
              <h4>Interested in joining us?</h4>
              <p>We're always looking for talented gamers and engineers.</p>
              <a href="/careers" className="btn btn-primary">View Careers</a>
            </div>
          </div>
        </div>
      </section>

      <footer className="about-footer">
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
              <a href="/careers">Careers</a>
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

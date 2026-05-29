import { useState } from 'react';
import './Contact.css';

const socialLinks = [
  {
    name: 'Discord',
    url: '#',
    username: 'magnetite.gg',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03z" />
      </svg>
    ),
  },
  {
    name: 'Twitter / X',
    url: '#',
    username: '@MagnetiteGG',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
      </svg>
    ),
  },
  {
    name: 'GitHub',
    url: '#',
    username: 'magnetite-gg',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
      </svg>
    ),
  },
  {
    name: 'Telegram',
    url: '#',
    username: '@magnetite',
    icon: (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.96 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z" />
      </svg>
    ),
  },
];

export default function Contact() {
  const [formData, setFormData] = useState({
    name: '',
    email: '',
    subject: '',
    message: '',
  });
  const [submitted, setSubmitted] = useState(false);

  const handleChange = (e) => {
    setFormData({ ...formData, [e.target.name]: e.target.value });
  };

  const handleSubmit = (e) => {
    e.preventDefault();
    setSubmitted(true);
  };

  return (
    <div className="contact-page">
      {/* ── Hero ───────────────────────────────────────────────────────────── */}
      <section className="contact-hero bg-atmosphere" aria-labelledby="contact-heading">
        <div className="magnetic-field" aria-hidden="true">
          <div className="field-line field-line-1" />
          <div className="field-line field-line-2" />
          <div className="field-line field-line-3" />
        </div>
        <div className="hero-content reveal">
          <span className="kicker reveal-1">// REACH US</span>
          <h1 id="contact-heading" className="hero-title reveal-2">
            Get in <span className="gradient-text">Touch</span>
          </h1>
          <p className="hero-subtitle reveal-3">
            Have questions about the platform, the SDK, or partnership opportunities?
            We&apos;d love to hear from you.
          </p>
        </div>
      </section>

      {/* ── Main content ───────────────────────────────────────────────────── */}
      <section className="contact-content">
        <div className="container">
          <div className="contact-grid">
            {/* Form */}
            <div className="contact-form-container">
              {!submitted ? (
                <form className="contact-form" onSubmit={handleSubmit} noValidate>
                  <span className="kicker">// SEND A MESSAGE</span>
                  <h2>Send us a message</h2>

                  <div className="form-group">
                    <label htmlFor="contact-name">Name</label>
                    <input
                      type="text"
                      id="contact-name"
                      name="name"
                      value={formData.name}
                      onChange={handleChange}
                      placeholder="Your name"
                      required
                      autoComplete="name"
                    />
                  </div>

                  <div className="form-group">
                    <label htmlFor="contact-email">Email</label>
                    <input
                      type="email"
                      id="contact-email"
                      name="email"
                      value={formData.email}
                      onChange={handleChange}
                      placeholder="you@example.com"
                      required
                      autoComplete="email"
                    />
                  </div>

                  <div className="form-group">
                    <label htmlFor="contact-subject">Subject</label>
                    <input
                      type="text"
                      id="contact-subject"
                      name="subject"
                      value={formData.subject}
                      onChange={handleChange}
                      placeholder="What&apos;s this about?"
                      required
                    />
                  </div>

                  <div className="form-group">
                    <label htmlFor="contact-message">Message</label>
                    <textarea
                      id="contact-message"
                      name="message"
                      value={formData.message}
                      onChange={handleChange}
                      placeholder="Your message..."
                      rows={6}
                      required
                    />
                  </div>

                  <button type="submit" className="btn btn-primary btn-lg submit-btn">
                    Send Message
                  </button>
                </form>
              ) : (
                <div className="success-message" role="status" aria-live="polite">
                  <div className="success-icon" aria-hidden="true">
                    <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  </div>
                  <h2>Message Sent!</h2>
                  <p>Thanks for reaching out. We&apos;ll get back to you within 24 hours.</p>
                  <button
                    className="btn btn-secondary"
                    onClick={() => {
                      setSubmitted(false);
                      setFormData({ name: '', email: '', subject: '', message: '' });
                    }}
                  >
                    Send Another Message
                  </button>
                </div>
              )}
            </div>

            {/* Sidebar */}
            <div className="contact-sidebar">
              <div className="sidebar-section">
                <span className="kicker">// RESPONSE TIME</span>
                <h3>Response Time</h3>
                <p>We typically respond within 24 hours during business days. For urgent matters, join our Discord community for faster support.</p>
              </div>

              <div className="sidebar-section">
                <span className="kicker">// LOCATION</span>
                <h3>Office</h3>
                <div className="office-info">
                  <div className="office-icon-wrap" aria-hidden="true">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <circle cx="12" cy="10" r="3" />
                      <path d="M12 2a8 8 0 0 1 8 8c0 5.4-7 12-8 12S4 15.4 4 10a8 8 0 0 1 8-8z" />
                    </svg>
                  </div>
                  <div>
                    <p className="office-label">Remote-first</p>
                    <p className="office-detail">Team members across 12 countries</p>
                  </div>
                </div>
              </div>

              <div className="sidebar-section">
                <span className="kicker">// SOCIAL</span>
                <h3>Connect</h3>
                <div className="social-links">
                  {socialLinks.map((link, i) => (
                    <a
                      key={i}
                      href={link.url}
                      className="social-link"
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      <span className="social-icon">{link.icon}</span>
                      <div className="social-info">
                        <span className="social-name">{link.name}</span>
                        <span className="social-username">{link.username}</span>
                      </div>
                    </a>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Footer ─────────────────────────────────────────────────────────── */}
      <footer className="contact-footer">
        <div className="container">
          <div className="footer-content">
            <div className="footer-brand">
              <div className="logo">
                <div className="logo-icon" aria-hidden="true">M</div>
                <span>Magnetite</span>
              </div>
              <p>Open-source Rust gaming. No middlemen.</p>
            </div>
            <nav className="footer-links" aria-label="Footer navigation">
              <a href="/marketplace">Marketplace</a>
              <a href="/about">About</a>
              <a href="/careers">Careers</a>
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

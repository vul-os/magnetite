import { useState } from 'react';
import './Contact.css';

const socialLinks = [
  { name: 'Discord', icon: '💬', url: '#', username: 'MagnetiteGG' },
  { name: 'Twitter', icon: '🐦', url: '#', username: '@MagnetiteGG' },
  { name: 'GitHub', icon: '🐙', url: '#', username: 'magnetite-gg' },
  { name: 'Telegram', icon: '✈️', url: '#', username: '@magnetite' },
];

export default function Contact() {
  const [formData, setFormData] = useState({
    name: '',
    email: '',
    subject: '',
    message: ''
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
      <section className="contact-hero">
        <div className="magnetic-field">
          <div className="field-line field-line-1"></div>
          <div className="field-line field-line-2"></div>
          <div className="field-line field-line-3"></div>
        </div>
        <div className="hero-content">
          <h1 className="hero-title">
            Get in <span className="gradient-text">Touch</span>
          </h1>
          <p className="hero-subtitle">
            Have questions? We'd love to hear from you. Send us a message and we'll
            respond as soon as possible.
          </p>
        </div>
      </section>

      <section className="contact-content">
        <div className="container">
          <div className="contact-grid">
            <div className="contact-form-container">
              {!submitted ? (
                <form className="contact-form" onSubmit={handleSubmit}>
                  <h2>Send us a message</h2>
                  <div className="form-group">
                    <label htmlFor="name">Name</label>
                    <input
                      type="text"
                      id="name"
                      name="name"
                      value={formData.name}
                      onChange={handleChange}
                      placeholder="Your name"
                      required
                    />
                  </div>
                  <div className="form-group">
                    <label htmlFor="email">Email</label>
                    <input
                      type="email"
                      id="email"
                      name="email"
                      value={formData.email}
                      onChange={handleChange}
                      placeholder="you@example.com"
                      required
                    />
                  </div>
                  <div className="form-group">
                    <label htmlFor="subject">Subject</label>
                    <input
                      type="text"
                      id="subject"
                      name="subject"
                      value={formData.subject}
                      onChange={handleChange}
                      placeholder="What's this about?"
                      required
                    />
                  </div>
                  <div className="form-group">
                    <label htmlFor="message">Message</label>
                    <textarea
                      id="message"
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
                <div className="success-message">
                  <div className="success-icon">✓</div>
                  <h2>Message Sent!</h2>
                  <p>Thanks for reaching out. We'll get back to you within 24 hours.</p>
                  <button
                    className="btn btn-secondary"
                    onClick={() => setSubmitted(false)}
                  >
                    Send Another Message
                  </button>
                </div>
              )}
            </div>

            <div className="contact-sidebar">
              <div className="sidebar-section">
                <h3>Response Time</h3>
                <p>We typically respond within 24 hours during business days. For urgent matters, check out our Discord community.</p>
              </div>

              <div className="sidebar-section">
                <h3>Office</h3>
                <div className="office-info">
                  <span className="office-icon">🌍</span>
                  <div>
                    <p className="office-label">Remote-first</p>
                    <p className="office-detail">Team members across 12 countries</p>
                  </div>
                </div>
              </div>

              <div className="sidebar-section">
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

      <footer className="contact-footer">
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

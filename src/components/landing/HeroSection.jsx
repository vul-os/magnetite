import { Button, Badge } from '../common';
import './Landing.css';

export default function HeroSection() {
  return (
    <section className="hero-section">
      <div className="hero-particles">
        {[...Array(20)].map((_, i) => (
          <div
            key={i}
            className="particle"
            style={{
              left: `${Math.random() * 100}%`,
              animationDelay: `${Math.random() * 5}s`,
              animationDuration: `${3 + Math.random() * 4}s`,
            }}
          />
        ))}
      </div>

      <div className="hero-field">
        {[...Array(5)].map((_, i) => (
          <div key={i} className={`field-line field-line-${i + 1}`} />
        ))}
      </div>

      <div className="container">
        <div className="hero-grid">
          <div className="hero-content">
            <Badge variant="subtle" color="green" size="md" dot>Now in Beta</Badge>
            <h1 className="hero-title">
              The Future of <span className="gradient-text">Gaming Infrastructure</span>
            </h1>
            <p className="hero-subtitle">
              Host your HTML5 games globally, earn crypto, and scale effortlessly.
              Magnetite powers the next generation of decentralized gaming.
            </p>
            <div className="hero-ctas">
              <Button size="lg" onClick={() => window.location.href = '/register'}>
                Get Started
              </Button>
              <Button size="lg" variant="secondary" onClick={() => window.location.href = '/marketplace'}>
                Explore Games
              </Button>
            </div>
            <div className="hero-stats">
              <div className="stat">
                <span className="stat-value">50K+</span>
                <span className="stat-label">Active Players</span>
              </div>
              <div className="stat">
                <span className="stat-value">200+</span>
                <span className="stat-label">Games Hosted</span>
              </div>
              <div className="stat">
                <span className="stat-value">$2M+</span>
                <span className="stat-label">Earnings Paid</span>
              </div>
            </div>
          </div>

          <div className="hero-visual">
            <div className="controller-float">
              <div className="magnet-ring ring-1" />
              <div className="magnet-ring ring-2" />
              <div className="magnet-ring ring-3" />
              <div className="controller-icon">
                <svg viewBox="0 0 24 24" fill="currentColor" width="80" height="80">
                  <path d="M21 6H3a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h18a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2zm-10 7H8v3H6v-3H3v-2h3V8h2v3h3v2zm9.5.5c0-.83.67-1.5 1.5-1.5s1.5.67 1.5 1.5-.67 1.5-1.5 1.5-1.5-.67-1.5-1.5zM15 16h2v-2h-2v2z"/>
                </svg>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

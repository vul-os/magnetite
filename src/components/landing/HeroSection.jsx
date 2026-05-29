import { useMemo } from 'react';
import { Button, Badge } from '../common';
import './Landing.css';

// Pre-compute particle positions outside the component to keep renders pure
const PARTICLES = Array.from({ length: 20 }, (_, i) => ({
  id: i,
  left: `${(i * 5.1 + 2.3) % 100}%`,
  delay: `${((i * 0.73) % 5).toFixed(2)}s`,
  duration: `${(3 + (i * 0.41) % 4).toFixed(2)}s`,
}));

function StatCountUp({ value, label }) {
  return (
    <div className="hero-stat">
      <span className="hero-stat-value">{value}</span>
      <span className="hero-stat-label">{label}</span>
    </div>
  );
}

export default function HeroSection() {
  const particles = useMemo(() => PARTICLES, []);

  return (
    <section className="hero-section" aria-labelledby="hero-heading">
      {/* Ambient particles */}
      <div className="hero-particles" aria-hidden="true">
        {particles.map((p) => (
          <div
            key={p.id}
            className="particle"
            style={{
              left: p.left,
              animationDelay: p.delay,
              animationDuration: p.duration,
            }}
          />
        ))}
      </div>

      {/* Magnetic field lines */}
      <div className="hero-field" aria-hidden="true">
        <div className="field-line field-line-1" />
        <div className="field-line field-line-2" />
        <div className="field-line field-line-3" />
        <div className="field-line field-line-4" />
        <div className="field-line field-line-5" />
      </div>

      <div className="container">
        <div className="hero-grid">
          <div className="hero-content">
            <span className="kicker">// BUILT IN RUST</span>
            <Badge variant="subtle" color="green" size="md" dot>
              Open Beta — Rust Games at Any Scale
            </Badge>

            <h1 id="hero-heading" className="hero-title">
              The Infrastructure Layer for{' '}
              <span className="gradient-text">Rust Games</span>
            </h1>

            <p className="hero-subtitle">
              From a weekend game-jam prototype to COD-scale AAA — Magnetite
              handles hosting, matchmaking, netcode, and monetization so you
              only write game logic in Rust.
            </p>

            <div className="hero-ctas">
              <Button
                size="lg"
                onClick={() => { window.location.href = '/register'; }}
              >
                Start Building
              </Button>
              <Button
                size="lg"
                variant="secondary"
                onClick={() => { window.location.href = '/marketplace'; }}
              >
                Explore Games
              </Button>
            </div>

            <div className="hero-stats">
              <StatCountUp value="2,847" label="Developers" />
              <div className="hero-stat-divider" aria-hidden="true" />
              <StatCountUp value="156+" label="Games Shipped" />
              <div className="hero-stat-divider" aria-hidden="true" />
              <StatCountUp value="$2.4M" label="USDC Paid Out" />
            </div>
          </div>

          <div className="hero-visual" aria-hidden="true">
            <div className="controller-float">
              <div className="magnet-ring ring-1" />
              <div className="magnet-ring ring-2" />
              <div className="magnet-ring ring-3" />
              <div className="controller-icon">
                {/* Rust crab icon */}
                <svg viewBox="0 0 120 120" fill="none" width="96" height="96">
                  <circle cx="60" cy="60" r="56" stroke="currentColor" strokeWidth="2" opacity="0.2" />
                  <text x="50%" y="54%" dominantBaseline="middle" textAnchor="middle" fontSize="60" fill="currentColor">
                    🦀
                  </text>
                </svg>
              </div>
            </div>

            {/* Terminal snippet */}
            <div className="hero-terminal">
              <div className="terminal-bar">
                <span className="t-dot t-red" />
                <span className="t-dot t-yellow" />
                <span className="t-dot t-green" />
                <span className="terminal-filename">Cargo.toml</span>
              </div>
              <pre className="terminal-body">
                <span className="t-comment"># Add Magnetite SDK</span>{'\n'}
                <span className="t-bracket">[dependencies]</span>{'\n'}
                <span className="t-key">magnetite-sdk</span>
                {' = '}
                <span className="t-string">&quot;0.8&quot;</span>{'\n'}
                <span className="t-key">bevy</span>
                {' = '}
                <span className="t-string">&quot;0.14&quot;</span>{'\n'}
                {'\n'}
                <span className="t-comment"># cargo run --target wasm32-unknown-unknown</span>
              </pre>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

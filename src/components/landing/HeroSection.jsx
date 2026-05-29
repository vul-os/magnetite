import { useMemo } from 'react';
import { Button, Badge } from '../common';
import './Landing.css';

// Pre-compute particle positions outside the component to keep renders pure
const PARTICLES = Array.from({ length: 24 }, (_, i) => ({
  id: i,
  left: `${(i * 4.17 + 1.5) % 100}%`,
  delay: `${((i * 0.73) % 6).toFixed(2)}s`,
  duration: `${(5 + (i * 0.41) % 5).toFixed(2)}s`,
  size: i % 3 === 0 ? '2px' : i % 3 === 1 ? '3px' : '1.5px',
}));

const STATS = [
  { value: '2,847', label: 'Developers' },
  { value: '156+',  label: 'Games Shipped' },
  { value: '$2.4M', label: 'USDC Paid Out' },
];

function StatItem({ value, label }) {
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
    <section className="hero-section bg-atmosphere" aria-labelledby="hero-heading">
      {/* Layered accent glows */}
      <div className="hero-glow-primary" aria-hidden="true" />
      <div className="hero-glow-secondary" aria-hidden="true" />

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
              width: p.size,
              height: p.size,
            }}
          />
        ))}
      </div>

      {/* Magnetic field lines */}
      <div className="hero-field" aria-hidden="true">
        {[1,2,3,4,5,6,7].map(n => (
          <div key={n} className={`field-line field-line-${n}`} />
        ))}
      </div>

      <div className="container">
        <div className="hero-grid">
          {/* Left: content with orchestrated reveal */}
          <div className="hero-content reveal">
            <div className="reveal-1">
              <span className="kicker">// BUILT IN RUST · OPEN SOURCE</span>
            </div>

            <div className="reveal-2">
              <Badge variant="subtle" color="green" size="md" dot>
                Open Beta — Rust Games at Any Scale
              </Badge>
            </div>

            <h1 id="hero-heading" className="hero-title reveal-3">
              The Infrastructure<br />
              Layer for{' '}
              <span className="gradient-text">Rust Games</span>
            </h1>

            <p className="hero-subtitle reveal-4">
              From a weekend game-jam prototype to COD-scale AAA — Magnetite
              handles hosting, matchmaking, netcode, and monetization so you
              only write game logic in Rust.
            </p>

            <div className="hero-ctas reveal-5">
              <Button
                size="lg"
                onClick={() => { window.location.href = '/register'; }}
              >
                Start Building Free
              </Button>
              <Button
                size="lg"
                variant="secondary"
                onClick={() => { window.location.href = '/marketplace'; }}
              >
                Explore Games
              </Button>
            </div>

            <div className="hero-trust reveal-5">
              <span className="trust-item">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>
                MIT License
              </span>
              <span className="trust-sep" aria-hidden="true" />
              <span className="trust-item">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
                USDC Payouts
              </span>
              <span className="trust-sep" aria-hidden="true" />
              <span className="trust-item">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
                15% Platform Fee
              </span>
            </div>

            <div className="hero-stats reveal-6">
              {STATS.map((s, i) => (
                <>
                  <StatItem key={s.label} value={s.value} label={s.label} />
                  {i < STATS.length - 1 && (
                    <div key={`sep-${i}`} className="hero-stat-divider" aria-hidden="true" />
                  )}
                </>
              ))}
            </div>
          </div>

          {/* Right: visual with staggered entrance */}
          <div className="hero-visual" aria-hidden="true">
            {/* Magnetic ring + icon */}
            <div className="magnetic-core reveal-4">
              <div className="magnet-ring ring-1" />
              <div className="magnet-ring ring-2" />
              <div className="magnet-ring ring-3" />
              <div className="magnet-ring ring-4" />
              <div className="core-icon">
                <svg viewBox="0 0 120 120" fill="none" width="88" height="88" aria-hidden="true">
                  <circle cx="60" cy="60" r="54" stroke="currentColor" strokeWidth="1.5" opacity="0.15" />
                  <circle cx="60" cy="60" r="38" stroke="currentColor" strokeWidth="1" opacity="0.1" />
                  <text x="50%" y="54%" dominantBaseline="middle" textAnchor="middle" fontSize="56" fill="currentColor">
                    🦀
                  </text>
                </svg>
              </div>
            </div>

            {/* Terminal snippet */}
            <div className="hero-terminal reveal-5">
              <div className="terminal-bar">
                <span className="t-dot t-red" />
                <span className="t-dot t-yellow" />
                <span className="t-dot t-green" />
                <span className="terminal-filename">game.rs</span>
              </div>
              <pre className="terminal-body">
                <span className="t-comment">// Magnetite game logic — compile to WASM or native</span>{'\n'}
                <span className="t-keyword">use</span>{' '}
                <span className="t-type">magnetite_sdk</span>
                <span className="t-bracket">::</span>
                <span className="t-bracket">{'{'}</span>
                <span className="t-key">GameLogic</span>
                <span className="t-bracket">, </span>
                <span className="t-key">Platform</span>
                <span className="t-bracket">{'}'}</span>
                <span className="t-punct">;</span>{'\n\n'}
                <span className="t-attr">#[derive(GameLogic)]</span>{'\n'}
                <span className="t-keyword">struct</span>{' '}
                <span className="t-type">MyGame</span>{' '}
                <span className="t-bracket">{'{'}</span>{'\n'}
                {'  '}
                <span className="t-key">platform</span>
                <span className="t-punct">: </span>
                <span className="t-type">Platform</span>
                <span className="t-punct">,</span>{'\n'}
                <span className="t-bracket">{'}'}</span>{'\n\n'}
                <span className="t-comment">// cargo build --target wasm32-unknown-unknown</span>
              </pre>
              <div className="terminal-status-bar">
                <span className="ts-item ts-lang">Rust</span>
                <span className="ts-item">UTF-8</span>
                <span className="ts-dot" />
                <span className="ts-item ts-ok">Compiling...</span>
              </div>
            </div>

            {/* Floating SDK chips */}
            <div className="hero-chips reveal-6">
              <div className="hero-chip chip-accent">Bevy 0.14</div>
              <div className="hero-chip chip-amber">WASM</div>
              <div className="hero-chip chip-info">Matchmaking</div>
              <div className="hero-chip chip-success">USDC</div>
            </div>
          </div>
        </div>
      </div>

      {/* Bottom fade to next section */}
      <div className="hero-bottom-fade" aria-hidden="true" />
    </section>
  );
}

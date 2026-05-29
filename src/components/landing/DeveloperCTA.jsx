import { CheckIcon } from '../../assets/icons';
import { Button } from '../common';
import './Landing.css';

const benefits = [
  '85% revenue share, paid weekly in USDC',
  'Zero upfront costs or hidden fees',
  'Server-authoritative netcode included',
  'WASM + native builds from one codebase',
  'Real-time analytics dashboard',
  'Paystack fiat on-ramp built in',
  'Custom domain & CNAME support',
  'MIT-licensed SDK, own your game forever',
];

export default function DeveloperCTA() {
  return (
    <section className="developer-cta-section" aria-labelledby="dev-cta-heading">
      <div className="developer-bg-pattern" aria-hidden="true" />
      <div className="container">
        <div className="developer-cta-grid">
          <div className="developer-cta-content">
            <span className="kicker">// FOR RUST DEVELOPERS</span>
            <h2 id="dev-cta-heading">
              Stop writing infrastructure.<br />
              Ship games.
            </h2>
            <p className="developer-subtitle">
              Magnetite handles hosting, netcode, matchmaking, payments, and
              analytics. Your only job is writing great Rust game logic.
            </p>
            <ul className="benefits-list" aria-label="Developer benefits">
              {benefits.map((benefit, index) => (
                <li key={index} className="benefit-item">
                  <CheckIcon className="check-icon" aria-hidden="true" />
                  <span>{benefit}</span>
                </li>
              ))}
            </ul>
            <Button
              size="lg"
              onClick={() => { window.location.href = '/register?type=developer'; }}
            >
              Start Publishing
            </Button>
          </div>

          <div className="developer-cta-visual">
            <div className="code-window">
              <div className="code-header">
                <div className="code-dot red" />
                <div className="code-dot yellow" />
                <div className="code-dot green" />
                <span className="code-title">game.rs</span>
              </div>
              <pre className="code-content">
{`use magnetite_sdk::prelude::*;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MagnetitePlugin {
            game_id: env!("MAGNETITE_GAME_ID"),
            // SDK handles: sessions, payments,
            // matchmaking, leaderboards, netcode
        })
        .add_systems(Update, game_logic)
        .run();
}

// Only write what matters: your game.
fn game_logic(/* ... */) { /* ... */ }`}
              </pre>
            </div>

            <div className="sdk-badges">
              <span className="sdk-badge">
                <span className="sdk-badge-dot" />
                Bevy 0.14
              </span>
              <span className="sdk-badge">
                <span className="sdk-badge-dot sdk-badge-dot--amber" />
                WASM Ready
              </span>
              <span className="sdk-badge">
                <span className="sdk-badge-dot sdk-badge-dot--info" />
                MIT License
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import './Welcome.css';

const QUICK_ACTIONS = [
  { icon: '⬡', label: 'Marketplace',  link: '/',                  description: 'Browse Rust games' },
  { icon: '⌘', label: 'Dev Studio',   link: '/developers/studio', description: 'Ship your game' },
  { icon: '◈', label: 'Friends',       link: '/friends',           description: 'Find teammates' },
  { icon: '◉', label: 'Leaderboard',  link: '/leaderboard',       description: 'View rankings' },
];

export default function Welcome() {
  return (
    <Layout>
      <div className="welcome-page">
        {/* ── Header ─────────────────────────────────────────── */}
        <header className="welcome-header">
          <span className="welcome-header-kicker reveal reveal-1">// WELCOME TO MAGNETITE</span>
          <h1 className="reveal reveal-2">You&apos;re all set.</h1>
          <p className="reveal reveal-3">
            Build, ship, and monetise Rust games — from game jam to AAA scale.
          </p>
        </header>

        {/* ── Quick Actions ───────────────────────────────────── */}
        <section className="quick-actions reveal reveal-4">
          <div className="section-heading">
            <h2>Quick Actions</h2>
            <span className="section-heading-kicker">navigate</span>
          </div>
          <div className="actions-grid">
            {QUICK_ACTIONS.map((action, index) => (
              <Link key={index} to={action.link} className="action-card">
                <div className="action-icon" aria-hidden="true">{action.icon}</div>
                <div className="action-label">{action.label}</div>
                <div className="action-description">{action.description}</div>
              </Link>
            ))}
          </div>
        </section>

        {/* ── CTA ────────────────────────────────────────────── */}
        <section className="welcome-cta reveal reveal-5">
          <span className="kicker">// GET STARTED</span>
          <h3>Ready to build?</h3>
          <p>Deploy your first Rust game to the Magnetite platform in minutes.</p>
          <Link to="/" className="btn btn-primary btn-lg">
            Browse Marketplace
          </Link>
        </section>
      </div>
    </Layout>
  );
}

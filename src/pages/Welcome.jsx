import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import './Welcome.css';

const RELEASE_NOTES = [
  {
    version: 'v1.2.0',
    date: 'May 2026',
    changes: [
      'New server-authoritative multiplayer matchmaking',
      'Reduced platform fee from 20% to 15%',
      'Achievement system with on-chain attestation',
      'WASM compile time down 40% via incremental builds',
    ]
  },
  {
    version: 'v1.1.0',
    date: 'April 2026',
    changes: [
      'Developer Dashboard with real-time session analytics',
      'USDC wallet integration via Circle',
      'Leaderboard rankings with anti-cheat validation',
    ]
  }
];

const QUICK_ACTIONS = [
  { icon: '⬡', label: 'Marketplace', link: '/', description: 'Browse Rust games' },
  { icon: '⌘', label: 'Dev Studio', link: '/developers/studio', description: 'Ship your game' },
  { icon: '◈', label: 'Friends', link: '/friends', description: 'Find teammates' },
  { icon: '◉', label: 'Leaderboard', link: '/leaderboard', description: 'View rankings' },
];

export default function Welcome() {
  return (
    <Layout>
      <div className="welcome-page">
        <header className="welcome-header">
          <span className="welcome-header-kicker">// WELCOME TO MAGNETITE</span>
          <h1>You&apos;re all set.</h1>
          <p>Build, ship, and monetize Rust games — from game jam to AAA scale.</p>
        </header>

        <section className="whats-new">
          <div className="section-heading">
            <h2>What&apos;s New</h2>
            <span className="section-heading-kicker">changelog</span>
          </div>
          <div className="release-notes">
            {RELEASE_NOTES.map((release, index) => (
              <div key={index} className="release-card">
                <div className="release-header">
                  <span className="release-version">{release.version}</span>
                  <span className="release-date">{release.date}</span>
                </div>
                <ul className="release-changes">
                  {release.changes.map((change, i) => (
                    <li key={i}>{change}</li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </section>

        <section className="quick-actions">
          <div className="section-heading">
            <h2>Quick Actions</h2>
            <span className="section-heading-kicker">navigate</span>
          </div>
          <div className="actions-grid">
            {QUICK_ACTIONS.map((action, index) => (
              <Link key={index} to={action.link} className="action-card">
                <div className="action-icon">{action.icon}</div>
                <div className="action-label">{action.label}</div>
                <div className="action-description">{action.description}</div>
              </Link>
            ))}
          </div>
        </section>

        <section className="welcome-cta">
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

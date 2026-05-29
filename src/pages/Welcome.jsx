import { Link } from 'react-router-dom';
import Layout from '../components/Layout';
import './Welcome.css';

const RELEASE_NOTES = [
  {
    version: 'v1.2.0',
    date: 'May 2026',
    changes: [
      'New multiplayer matchmaking system',
      'Reduced transaction fees by 50%',
      'Added achievement system',
      'Improved game loading times'
    ]
  },
  {
    version: 'v1.1.0',
    date: 'April 2026',
    changes: [
      'Launched developer dashboard',
      'Added USDC wallet integration',
      'Introduced leaderboard rankings'
    ]
  }
];

const QUICK_ACTIONS = [
  { icon: '🎮', label: 'Browse Games', link: '/', description: 'Find your next game' },
  { icon: '💼', label: 'Developer Studio', link: '/developers/studio', description: 'Host your game' },
  { icon: '👥', label: 'Friends', link: '/friends', description: 'Connect with players' },
  { icon: '🏆', label: 'Leaderboard', link: '/leaderboard', description: 'View rankings' },
];

export default function Welcome() {
  return (
    <Layout>
      <div className="welcome-page">
        <header className="welcome-header">
          <h1>Welcome to Magnetite</h1>
          <p>You're all set! Here's what's new.</p>
        </header>

        <section className="whats-new">
          <h2>What's New</h2>
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
          <h2>Quick Actions</h2>
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
          <h3>Ready to play?</h3>
          <p>Start browsing thousands of open source games.</p>
          <Link to="/" className="btn btn-primary btn-lg">
            Browse Marketplace
          </Link>
        </section>
      </div>
    </Layout>
  );
}

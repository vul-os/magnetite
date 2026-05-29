import { CheckIcon } from '../../assets/icons';
import { Button } from '../common';
import './Landing.css';

const benefits = [
  'Revenue share: 85% to developers',
  'No upfront costs or hidden fees',
  'Instant payouts in USDC',
  'Usage-based scaling automatically',
  'Full REST API access',
  'Real-time analytics dashboard',
  'Custom domain support',
  'WebSocket for live game state',
];

export default function DeveloperCTA() {
  return (
    <section className="developer-cta-section">
      <div className="developer-bg-pattern" />
      <div className="container">
        <div className="developer-cta-grid">
          <div className="developer-cta-content">
            <h2>For Game Developers</h2>
            <p className="developer-subtitle">
              Stop worrying about infrastructure costs. Focus on building great games while Magnetite handles the rest.
            </p>
            <ul className="benefits-list">
              {benefits.map((benefit, index) => (
                <li key={index} className="benefit-item">
                  <CheckIcon className="check-icon" />
                  <span>{benefit}</span>
                </li>
              ))}
            </ul>
            <Button size="lg" onClick={() => window.location.href = '/register?type=developer'}>
              Start Hosting
            </Button>
          </div>
          <div className="developer-cta-visual">
            <div className="code-window">
              <div className="code-header">
                <div className="code-dot red" />
                <div className="code-dot yellow" />
                <div className="code-dot green" />
                <span className="code-title">deploy.ts</span>
              </div>
              <pre className="code-content">
{`import { Magnetite } from '@magnetite/sdk';

const game = await Magnetite.create({
  repo: 'my-org/my-game',
  pricing: {
    perSession: 0.05, // USDC
  },
  maxPlayers: 100,
});

await game.deploy();
console.log(\`Deployed at \${game.url}\`);`}
              </pre>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

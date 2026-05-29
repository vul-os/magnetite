import { WalletIcon, UsersIcon, TrophyIcon, ZapIcon, GlobeIcon, CodeIcon } from '../../assets/icons';
import { Card, CardBody } from '../common';
import './Landing.css';

const features = [
  {
    icon: WalletIcon,
    title: 'Earn Crypto',
    description: 'Host games and earn USDC for every session played. Withdraw earnings instantly to your wallet.',
  },
  {
    icon: GlobeIcon,
    title: 'Global Edge Network',
    description: 'Your games run on decentralized infrastructure across 50+ edge locations worldwide.',
  },
  {
    icon: ZapIcon,
    title: 'Zero Configuration',
    description: 'Connect your GitHub repo and deploy in seconds. No servers to manage, no infrastructure to maintain.',
  },
  {
    icon: UsersIcon,
    title: 'Built-in Matchmaking',
    description: 'Automatic player matching with skill-based grouping. Fill your games with players effortlessly.',
  },
  {
    icon: TrophyIcon,
    title: 'Leaderboards & Achievements',
    description: 'Leaderboards and achievements keep players engaged and coming back for more.',
  },
  {
    icon: CodeIcon,
    title: 'Developer First',
    description: 'Simple REST API, comprehensive SDKs, and detailed analytics dashboard for game developers.',
  },
];

export default function FeaturesSection() {
  return (
    <section className="features-section">
      <div className="container">
        <h2 className="section-title">
          Everything You Need to <span className="gradient-text">Scale</span>
        </h2>
        <div className="features-grid">
          {features.map((feature, index) => (
            <Card key={index} variant="interactive" padding="lg">
              <CardBody>
                <div className="feature-icon-wrapper">
                  <feature.icon className="feature-icon" />
                </div>
                <h3>{feature.title}</h3>
                <p>{feature.description}</p>
              </CardBody>
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}

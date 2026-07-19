import { WalletIcon, UsersIcon, TrophyIcon, ZapIcon, GlobeIcon, CodeIcon } from '../../assets/icons';
import './Landing.css';

const features = [
  {
    Icon: CodeIcon,
    kicker: '// RUST-FIRST',
    title: 'Author in Rust, Ship Everywhere',
    description:
      'Write game logic in Rust once. Magnetite compiles your Bevy project to WASM for browsers and native for desktop — zero config.',
  },
  {
    Icon: GlobeIcon,
    kicker: '// EDGE NETWORK',
    title: 'Global Low-Latency Hosting',
    description:
      'Server-authoritative Rust processes run across 50+ edge regions. Sub-20 ms RTT for players worldwide, sandboxed and scalable.',
  },
  {
    Icon: ZapIcon,
    kicker: '// ONE COMMAND',
    title: 'Deploy from GitHub in Seconds',
    description:
      'Connect your repository, set pricing, push. Magnetite builds, tests, and deploys your game automatically on every push.',
  },
  {
    Icon: UsersIcon,
    kicker: '// NETCODE BUILT-IN',
    title: 'Real-Time Multiplayer & Matchmaking',
    description:
      'Integrated game-server netcode, lobby management, and skill-based matchmaking — ready to use from your SDK call.',
  },
  {
    Icon: TrophyIcon,
    kicker: '// ENGAGEMENT',
    title: 'Leaderboards & Achievements',
    description:
      'Platform-wide persistence: leaderboards, achievements, and player stats work the same from a jam game to a AAA title.',
  },
  {
    Icon: WalletIcon,
    kicker: '// MONETIZATION',
    title: 'Paid Directly, Non-Custodially',
    description:
      'Buyers pay your wallet in the same atomic transaction that mints a signed receipt. Nobody holds your money, so there is no payout to request and no processing delay — and the protocol fee defaults to 0 bps.',
  },
];

export default function FeaturesSection() {
  return (
    <section className="features-section" aria-labelledby="features-heading">
      <div className="container">
        <div className="section-header-centered">
          <span className="kicker">// PLATFORM CAPABILITIES</span>
          <h2 id="features-heading" className="section-heading">
            Everything you need to{' '}
            <span className="gradient-text">scale</span>
          </h2>
          <p className="section-lead">
            Magnetite provides the full stack — infrastructure, SDK, storefront, and payments.
            You provide the Rust game logic.
          </p>
        </div>

        <div className="features-grid">
          {features.map(({ Icon, kicker, title, description }, index) => (
            <div key={index} className="feature-card">
              <span className="feature-kicker">{kicker}</span>
              <div className="feature-icon-wrapper" aria-hidden="true">
                <Icon className="feature-icon" />
              </div>
              <h3 className="feature-title">{title}</h3>
              <p className="feature-desc">{description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

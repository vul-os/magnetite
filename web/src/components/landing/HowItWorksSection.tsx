import { GithubIcon, UploadIcon, ChartIcon, ChevronRightIcon } from '../../assets/icons';
import './Landing.css';

const steps = [
  {
    number: '01',
    kicker: '// CONNECT',
    title: 'Link your Rust Repo',
    description:
      'Connect your GitHub repository containing your Bevy game. Magnetite detects Cargo.toml and sets up the build pipeline automatically.',
    Icon: GithubIcon,
  },
  {
    number: '02',
    kicker: '// CONFIGURE',
    title: 'Set Pricing & Deploy',
    description:
      'Define your per-session price in USDC, player limits, and server region. One push to GitHub triggers a full WASM + native build and deploys to the edge.',
    Icon: UploadIcon,
  },
  {
    number: '03',
    kicker: '// EARN',
    title: 'Get Paid Wallet-to-Wallet',
    description:
      'Buyers pay your wallet directly in USDC and each sale mints a signed receipt you can verify. Nothing is held in escrow, the platform takes no cut, and there is no payout queue to wait on.',
    Icon: ChartIcon,
  },
];

export default function HowItWorksSection() {
  return (
    <section className="how-it-works-section" aria-labelledby="how-heading">
      <div className="container">
        <div className="section-header-centered sr">
          <span className="kicker">// WORKFLOW</span>
          <h2 id="how-heading" className="section-heading">
            From code to{' '}
            <span className="gradient-text">live game</span>
          </h2>
          <p className="section-lead">
            Three steps from your Rust codebase to a globally hosted, monetized game
            with real players.
          </p>
        </div>

        <div className="steps-container sr sr-group">
          {steps.map(({ number, kicker, title, description, Icon }, index) => (
            <div key={index} className="step-item spot">
              <div className="step-number">{number}</div>
              <span className="step-kicker">{kicker}</span>
              <div className="step-icon-wrapper" aria-hidden="true">
                <Icon className="step-icon" />
              </div>
              <h3 className="step-title">{title}</h3>
              <p className="step-desc">{description}</p>
              {index < steps.length - 1 && (
                <div className="step-connector" aria-hidden="true">
                  <ChevronRightIcon className="connector-arrow" />
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

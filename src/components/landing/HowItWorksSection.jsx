import { GithubIcon, UploadIcon, ChartIcon, ChevronRightIcon } from '../../assets/icons';
import './Landing.css';

const steps = [
  {
    number: '01',
    title: 'Connect Your Repo',
    description: 'Link your GitHub repository containing your HTML5 game. We support any framework or vanilla JavaScript.',
    icon: GithubIcon,
  },
  {
    number: '02',
    title: 'Configure & Deploy',
    description: 'Set your pricing per session, game parameters, and player limits. One-click deployment to our edge network.',
    icon: UploadIcon,
  },
  {
    number: '03',
    title: 'Track & Earn',
    description: 'Monitor live analytics, player sessions, and earnings in real-time. Withdraw your USDC anytime.',
    icon: ChartIcon,
  },
];

export default function HowItWorksSection() {
  return (
    <section className="how-it-works-section">
      <div className="container">
        <h2 className="section-title">
          How It <span className="gradient-text">Works</span>
        </h2>
        <div className="steps-container">
          {steps.map((step, index) => (
            <div key={index} className="step-item">
              <div className="step-number">{step.number}</div>
              <div className="step-icon-wrapper">
                <step.icon className="step-icon" />
              </div>
              <h3>{step.title}</h3>
              <p>{step.description}</p>
              {index < steps.length - 1 && (
                <div className="step-connector">
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

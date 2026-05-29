import { Button } from '../common';
import './Landing.css';

export default function FinalCTA() {
  return (
    <section className="final-cta-section">
      <div className="cta-glow" />
      <div className="container">
        <div className="final-cta-content">
          <h2>Ready to Build the Future of Gaming?</h2>
          <p>Join thousands of developers already earning on Magnetite. Your first game could be live in minutes.</p>
          <Button size="lg" onClick={() => window.location.href = '/register'}>
            Start Building Today
          </Button>
          <div className="cta-trust">
            <span>No credit card required</span>
            <span className="separator">|</span>
            <span>Deploy in under 5 minutes</span>
            <span className="separator">|</span>
            <span>85% revenue share</span>
          </div>
        </div>
      </div>
    </section>
  );
}

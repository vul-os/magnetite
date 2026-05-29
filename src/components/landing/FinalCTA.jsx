import { Button } from '../common';
import './Landing.css';

export default function FinalCTA() {
  return (
    <section className="final-cta-section" aria-labelledby="final-cta-heading">
      <div className="cta-glow" aria-hidden="true" />
      <div className="container">
        <div className="final-cta-content">
          <span className="kicker">// GET STARTED</span>
          <h2 id="final-cta-heading">
            Ready to ship your Rust game<br />to the world?
          </h2>
          <p>
            Join developers already building on Magnetite. Your first game could be
            live in minutes — from a game-jam prototype to a AAA-scale title.
          </p>

          <div className="final-cta-buttons">
            <Button
              size="lg"
              onClick={() => { window.location.href = '/register'; }}
            >
              Start Building Today
            </Button>
            <Button
              size="lg"
              variant="secondary"
              onClick={() => { window.location.href = '/docs'; }}
            >
              Read the Docs
            </Button>
          </div>

          <div className="cta-trust" aria-label="Trust indicators">
            <span>MIT licensed</span>
            <span className="separator" aria-hidden="true">·</span>
            <span>No credit card required</span>
            <span className="separator" aria-hidden="true">·</span>
            <span>85% revenue share</span>
            <span className="separator" aria-hidden="true">·</span>
            <span>Deploy in under 5 min</span>
          </div>
        </div>
      </div>
    </section>
  );
}

import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import Button from '../components/common/Button';
import OnboardingProgress from '../components/OnboardingProgress';
import GameCard from '../components/GameCard';
import { api } from '../api/client';
import { shortKey } from '../utils/currency';
import magnetiteLogo from '../assets/magnetite-logo.svg';
import './Onboarding.css';

const ONBOARDING_STORAGE_KEY = 'magnetite_onboarding_completed';

// NON-CUSTODIAL (seam §3.6 `PaymentRail`): there is no "add funds" step because
// this node never holds funds. A wallet is an address you control; a purchase
// pays the developer/operator wallet directly and mints a signed receipt.
const STEPS = ['Welcome', 'Link Wallet', 'Browse Games'];

// Mock-only sample games. Gated on VITE_USE_MOCKS === 'true' (strict) so these
// invented titles never reach a real first paint — production shows only games
// the API actually returns. See DESIGN.md §7.
const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';
const FEATURED_GAMES_MOCK = [
  { id: 1, title: 'Cosmic Raiders',   developer: 'StarForge Studios', fee_per_session: 0.50, category: 'Action',  thumbnail: 'https://picsum.photos/seed/game1/400/225' },
  { id: 2, title: 'Puzzle Dimension', developer: 'MindBend Games',    fee_per_session: 0.25, category: 'Puzzle',  thumbnail: 'https://picsum.photos/seed/game2/400/225' },
  { id: 3, title: 'Speed Legends',    developer: 'Velocity Labs',     fee_per_session: 0.75, category: 'Racing',  thumbnail: 'https://picsum.photos/seed/game3/400/225' },
];

/** A wallet address is a 32-byte hex Ed25519 public key. */
const HEX_KEY_RE = /^[0-9a-fA-F]{64}$/;

function WelcomeStep({ onNext }) {
  return (
    <div className="onboarding-step welcome-step">
      <div className="welcome-visual">
        <img src={magnetiteLogo} className="welcome-icon" aria-hidden="true" alt="" />
        <div className="welcome-glow" aria-hidden="true"></div>
      </div>
      <span className="step-kicker">// BUILT IN RUST</span>
      <h1>Welcome to Magnetite</h1>
      <p className="welcome-subtitle">
        The open-source platform for building, distributing, and monetising
        Rust games — from a weekend game jam to a live-service title.
      </p>
      <div className="welcome-features">
        <div className="welcome-feature">
          <span className="feature-icon" aria-hidden="true">⚡</span>
          <span>Play Rust games compiled to WASM</span>
        </div>
        <div className="welcome-feature">
          <span className="feature-icon" aria-hidden="true">◈</span>
          <span>Pay in USDC from a wallet you control — we never hold your funds</span>
        </div>
        <div className="welcome-feature">
          <span className="feature-icon" aria-hidden="true">◉</span>
          <span>Developers are paid wallet-to-wallet — the platform takes no cut</span>
        </div>
      </div>
      <Button size="lg" onClick={onNext}>
        Get Started
      </Button>
    </div>
  );
}

function WalletStep({ onNext, onSkip }) {
  const [input, setInput]           = useState('');
  const [linking, setLinking]       = useState(false);
  const [linked, setLinked]         = useState(null);
  const [error, setError]           = useState(null);

  const clean = input.trim().replace(/^0x/, '');
  const valid = HEX_KEY_RE.test(clean);

  const linkWallet = async () => {
    if (!valid) {
      setError('Enter a 32-byte hex Ed25519 public key (64 hex characters).');
      return;
    }
    setLinking(true);
    setError(null);
    try {
      const data = await api.wallet.link(clean);
      const payload = data?.data ?? data;
      setLinked(payload?.wallet_address ?? clean.toLowerCase());
    } catch (err) {
      setError(err.message || 'Could not link that wallet. You can link one later in Wallet settings.');
    } finally {
      setLinking(false);
    }
  };

  return (
    <div className="onboarding-step wallet-step">
      <h2>Link Your Wallet</h2>
      <p className="step-description">
        Magnetite is non-custodial: your wallet is an address <em>you</em> control,
        not a balance we hold. Buying an item pays the developer&apos;s wallet
        directly and mints a signed receipt — that receipt is your entitlement.
        You only need a wallet when you want to buy something, so you can skip this.
      </p>

      {error && (
        <div className="step-error" role="alert">{error}</div>
      )}

      {!linked ? (
        <div className="wallet-link-form">
          <label className="address-label" htmlFor="onboarding-wallet-address">
            Wallet Address
          </label>
          <div className="wallet-key-input">
            <input
              id="onboarding-wallet-address"
              type="text"
              spellCheck="false"
              autoComplete="off"
              placeholder="32-byte hex Ed25519 public key"
              value={input}
              onChange={(e) => { setInput(e.target.value); setError(null); }}
              onKeyDown={(e) => e.key === 'Enter' && !linking && linkWallet()}
              aria-describedby="onboarding-wallet-hint"
            />
          </div>
          <p id="onboarding-wallet-hint" className="wallet-key-hint">
            64 hex characters. Nothing is custodied — settlement is USDC, wallet to wallet.
          </p>
          <Button onClick={linkWallet} loading={linking} disabled={!valid || linking}>
            Link Wallet
          </Button>
        </div>
      ) : (
        <div className="wallet-address-display">
          <div className="address-label">Linked Wallet</div>
          <div className="address-value">{linked}</div>
          <div className="address-actions">
            <Button variant="ghost" size="sm" onClick={() => navigator.clipboard.writeText(linked)}>
              Copy {shortKey(linked)}
            </Button>
          </div>
        </div>
      )}

      <div className="step-actions">
        <Button onClick={onNext} disabled={linking}>
          Continue
        </Button>
        <Button variant="ghost" onClick={onSkip}>
          Skip for now
        </Button>
      </div>
    </div>
  );
}

function BrowseGamesStep({ onComplete, featuredGames }) {
  return (
    <div className="onboarding-step browse-step">
      <h2>Discover Games</h2>
      <p className="step-description">
        Browse our marketplace of open-source Rust games. Start with these featured titles.
      </p>

      {featuredGames.length > 0 ? (
        <div className="featured-games-grid">
          {featuredGames.map(game => (
            <GameCard key={game.id} game={game} showPlayButton={false} />
          ))}
        </div>
      ) : (
        <div className="state state-empty">
          <p className="state-body">
            No games have been published yet. New titles will appear here as
            developers ship them.
          </p>
        </div>
      )}

      <div className="step-actions">
        <Button size="lg" onClick={onComplete}>
          Browse Marketplace
        </Button>
        <Button variant="ghost" onClick={onComplete}>
          Skip to dashboard
        </Button>
      </div>
    </div>
  );
}

export default function Onboarding() {
  const [currentStep, setCurrentStep]       = useState(0);
  const [featuredGames, setFeaturedGames]   = useState(USE_MOCKS ? FEATURED_GAMES_MOCK : []);
  const navigate = useNavigate();

  useEffect(() => {
    const completed = localStorage.getItem(ONBOARDING_STORAGE_KEY);
    if (completed === 'true') {
      navigate('/');
    }
  }, [navigate]);

  // Pre-fetch real featured games. On empty or failure we show an honest empty
  // state rather than substituting invented data (DESIGN.md §7).
  useEffect(() => {
    api.games.list()
      .then(data => {
        const list = Array.isArray(data) ? data : (data?.games ?? null);
        if (list && list.length > 0) {
          setFeaturedGames(list.slice(0, 3));
        }
      })
      .catch(() => { /* keep whatever we have; no invented fallback */ });
  }, []);

  const completeOnboarding = () => {
    localStorage.setItem(ONBOARDING_STORAGE_KEY, 'true');
    navigate('/');
  };

  const handleNext = () => {
    if (currentStep < STEPS.length - 1) {
      setCurrentStep(currentStep + 1);
    } else {
      completeOnboarding();
    }
  };

  const handleSkip = () => {
    handleNext();
  };

  const renderStep = () => {
    switch (currentStep) {
      case 0:
        return <WelcomeStep onNext={handleNext} />;
      case 1:
        return <WalletStep onNext={handleNext} onSkip={handleSkip} />;
      case 2:
        return <BrowseGamesStep onComplete={completeOnboarding} featuredGames={featuredGames} />;
      default:
        return null;
    }
  };

  return (
    <div className="onboarding-page">
      <div className="onboarding-container">
        <OnboardingProgress currentStep={currentStep} totalSteps={STEPS.length} />
        <div className="onboarding-content">
          {renderStep()}
        </div>
      </div>
    </div>
  );
}

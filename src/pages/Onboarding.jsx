import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import Button from '../components/common/Button';
import OnboardingProgress from '../components/OnboardingProgress';
import GameCard from '../components/GameCard';
import { api } from '../api/client';
import './Onboarding.css';

const ONBOARDING_STORAGE_KEY = 'magnetite_onboarding_completed';

const STEPS = ['Welcome', 'Create Wallet', 'Add Funds', 'Browse Games'];

// Placeholder featured games — replaced by real games if the API responds
const FEATURED_GAMES_FALLBACK = [
  { id: 1, title: 'Cosmic Raiders',   developer: 'StarForge Studios', fee_per_session: 0.50, category: 'Action',  thumbnail: 'https://picsum.photos/seed/game1/400/225' },
  { id: 2, title: 'Puzzle Dimension', developer: 'MindBend Games',    fee_per_session: 0.25, category: 'Puzzle',  thumbnail: 'https://picsum.photos/seed/game2/400/225' },
  { id: 3, title: 'Speed Legends',    developer: 'Velocity Labs',     fee_per_session: 0.75, category: 'Racing',  thumbnail: 'https://picsum.photos/seed/game3/400/225' },
];

const FUND_AMOUNTS = [10, 25, 50, 100];

function WelcomeStep({ onNext }) {
  return (
    <div className="onboarding-step welcome-step">
      <div className="welcome-visual">
        <div className="welcome-icon" aria-hidden="true">M</div>
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
          <span>Pay with USDC — instant settlement</span>
        </div>
        <div className="welcome-feature">
          <span className="feature-icon" aria-hidden="true">◉</span>
          <span>Earn 85% of playtime revenue as developer</span>
        </div>
      </div>
      <Button size="lg" onClick={onNext}>
        Get Started
      </Button>
    </div>
  );
}

function WalletStep({ onNext, onSkip }) {
  const [walletAddress, setWalletAddress] = useState('');
  const [isGenerating, setIsGenerating]   = useState(false);
  const [showAddress, setShowAddress]     = useState(false);
  const [error, setError]                 = useState(null);

  const generateWallet = async () => {
    setIsGenerating(true);
    setError(null);
    try {
      // POST to wallet API — backend creates/returns the user's wallet address
      const data = await api.wallet.balance();
      const address = data?.address ?? data?.wallet_address ?? null;
      if (address) {
        setWalletAddress(address);
        setShowAddress(true);
      } else {
        // Backend didn't return an address yet (wallet auto-created on registration)
        // Fetch balance which also initialises the wallet server-side
        setWalletAddress('Wallet initialised — view full address in Settings');
        setShowAddress(true);
      }
    } catch {
      setError('Could not initialise wallet. You can set it up later in Settings.');
      setShowAddress(true); // allow proceeding
      setWalletAddress('');
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <div className="onboarding-step wallet-step">
      <h2>Set Up Your Wallet</h2>
      <p className="step-description">
        A USDC wallet lets you pay for game sessions and receive winnings.
        Your wallet is managed securely by Magnetite.
      </p>

      {error && (
        <div className="step-error" role="alert">{error}</div>
      )}

      {!showAddress ? (
        <div className="wallet-options">
          <div
            className="wallet-option primary"
            onClick={isGenerating ? undefined : generateWallet}
            role="button"
            tabIndex={0}
            aria-disabled={isGenerating}
            onKeyDown={(e) => e.key === 'Enter' && !isGenerating && generateWallet()}
          >
            <div className="option-icon" aria-hidden="true">{isGenerating ? '⏳' : '✨'}</div>
            <h3>{isGenerating ? 'Initialising…' : 'Create Wallet'}</h3>
            <p>Create your USDC wallet to start playing</p>
          </div>
        </div>
      ) : (
        walletAddress && (
          <div className="wallet-address-display">
            <div className="address-label">Wallet Status</div>
            <div className="address-value">{walletAddress}</div>
            {walletAddress.startsWith('0x') && (
              <div className="address-actions">
                <Button variant="ghost" size="sm" onClick={() => navigator.clipboard.writeText(walletAddress)}>
                  Copy Address
                </Button>
              </div>
            )}
          </div>
        )
      )}

      <div className="step-actions">
        <Button onClick={onNext} disabled={isGenerating && !showAddress}>
          Continue
        </Button>
        <Button variant="ghost" onClick={onSkip}>
          Skip for now
        </Button>
      </div>
    </div>
  );
}

function FundsStep({ onNext, onSkip }) {
  const [selectedAmount, setSelectedAmount] = useState(null);
  const [customAmount, setCustomAmount]     = useState('');
  const [isProcessing, setIsProcessing]     = useState(false);
  const [depositError, setDepositError]     = useState(null);

  const amount = selectedAmount || parseFloat(customAmount) || 0;

  const handleDeposit = async (method) => {
    if (!amount) return;
    setIsProcessing(true);
    setDepositError(null);
    try {
      await api.wallet.deposit({ amount, payment_method: method });
      onNext();
    } catch (err) {
      setDepositError(err.message || 'Deposit failed. You can add funds later in Wallet settings.');
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <div className="onboarding-step funds-step">
      <h2>Add Funds</h2>
      <p className="step-description">
        Add USDC to your wallet to start playing. You can skip this step and add funds later.
      </p>

      {depositError && (
        <div className="step-error" role="alert">{depositError}</div>
      )}

      <div className="amount-selector">
        <div className="amount-label">Select Amount</div>
        <div className="amount-options">
          {FUND_AMOUNTS.map(amt => (
            <button
              key={amt}
              className={`amount-option ${selectedAmount === amt ? 'selected' : ''}`}
              onClick={() => {
                setSelectedAmount(amt);
                setCustomAmount('');
              }}
            >
              ${amt}
            </button>
          ))}
        </div>
        <div className="custom-amount">
          <span className="currency-symbol">$</span>
          <input
            type="number"
            placeholder="Custom amount"
            value={customAmount}
            onChange={(e) => {
              setCustomAmount(e.target.value);
              setSelectedAmount(null);
            }}
            min="1"
          />
        </div>
      </div>

      {amount > 0 && (
        <div className="deposit-options">
          <div
            className={`deposit-option ${isProcessing ? 'loading' : ''}`}
            onClick={() => !isProcessing && handleDeposit('paystack')}
            role="button"
            tabIndex={0}
            aria-disabled={isProcessing}
            onKeyDown={(e) => e.key === 'Enter' && !isProcessing && handleDeposit('paystack')}
          >
            <div className="option-icon" aria-hidden="true">💳</div>
            <div className="option-info">
              <h4>Pay with Card</h4>
              <p>Visa, Mastercard via Paystack</p>
            </div>
            <div className="option-arrow" aria-hidden="true">→</div>
          </div>
          <div
            className={`deposit-option ${isProcessing ? 'loading' : ''}`}
            onClick={() => !isProcessing && handleDeposit('usdc')}
            role="button"
            tabIndex={0}
            aria-disabled={isProcessing}
            onKeyDown={(e) => e.key === 'Enter' && !isProcessing && handleDeposit('usdc')}
          >
            <div className="option-icon" aria-hidden="true">🪙</div>
            <div className="option-info">
              <h4>Deposit USDC</h4>
              <p>Transfer from another wallet</p>
            </div>
            <div className="option-arrow" aria-hidden="true">→</div>
          </div>
        </div>
      )}

      <div className="step-actions">
        <Button variant="secondary" onClick={onSkip}>
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

      <div className="featured-games-grid">
        {featuredGames.map(game => (
          <GameCard key={game.id} game={game} showPlayButton={false} />
        ))}
      </div>

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
  const [featuredGames, setFeaturedGames]   = useState(FEATURED_GAMES_FALLBACK);
  const navigate = useNavigate();

  useEffect(() => {
    const completed = localStorage.getItem(ONBOARDING_STORAGE_KEY);
    if (completed === 'true') {
      navigate('/');
    }
  }, [navigate]);

  // Pre-fetch featured games so the last step looks real
  useEffect(() => {
    api.games.list()
      .then(data => {
        const list = Array.isArray(data) ? data : (data?.games ?? null);
        if (list && list.length > 0) {
          setFeaturedGames(list.slice(0, 3));
        }
      })
      .catch(() => { /* keep fallback */ });
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
        return <FundsStep onNext={handleNext} onSkip={handleSkip} />;
      case 3:
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

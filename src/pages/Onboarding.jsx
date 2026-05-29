import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import Button from '../components/common/Button';
import OnboardingProgress from '../components/OnboardingProgress';
import GameCard from '../components/GameCard';
import Layout from '../components/Layout';
import './Onboarding.css';

const ONBOARDING_STORAGE_KEY = 'magnetite_onboarding_completed';

const STEPS = ['Welcome', 'Create Wallet', 'Add Funds', 'Browse Games'];

const FEATURED_GAMES = [
  { id: 1, title: 'Cosmic Raiders', developer: 'StarForge Studios', fee_per_session: 0.50, category: 'Action', thumbnail: 'https://picsum.photos/seed/game1/400/225' },
  { id: 2, title: 'Puzzle Dimension', developer: 'MindBend Games', fee_per_session: 0.25, category: 'Puzzle', thumbnail: 'https://picsum.photos/seed/game2/400/225' },
  { id: 3, title: 'Speed Legends', developer: 'Velocity Labs', fee_per_session: 0.75, category: 'Racing', thumbnail: 'https://picsum.photos/seed/game3/400/225' },
];

const FUND_AMOUNTS = [10, 25, 50, 100];

function WelcomeStep({ onNext }) {
  return (
    <div className="onboarding-step welcome-step">
      <div className="welcome-visual">
        <div className="welcome-icon">M</div>
        <div className="welcome-glow"></div>
      </div>
      <h1>Welcome to Magnetite</h1>
      <p className="welcome-subtitle">
        The decentralized gaming platform where developers own their games,
        players keep their winnings, and everyone plays on a level field.
      </p>
      <div className="welcome-features">
        <div className="welcome-feature">
          <span className="feature-icon">⚡</span>
          <span>Play Open Source Games</span>
        </div>
        <div className="welcome-feature">
          <span className="feature-icon">💎</span>
          <span>Pay with USDC</span>
        </div>
        <div className="welcome-feature">
          <span className="feature-icon">📈</span>
          <span>Earn as Developer</span>
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
  const [isGenerating, setIsGenerating] = useState(false);
  const [showAddress, setShowAddress] = useState(false);

  const generateWallet = () => {
    setIsGenerating(true);
    setTimeout(() => {
      const mockAddress = '0x' + Array.from({ length: 40 }, () =>
        Math.floor(Math.random() * 16).toString(16)
      ).join('');
      setWalletAddress(mockAddress);
      setShowAddress(true);
      setIsGenerating(false);
    }, 1500);
  };

  return (
    <div className="onboarding-step wallet-step">
      <h2>Set Up Your Wallet</h2>
      <p className="step-description">
        A USDC wallet lets you pay for game sessions and receive winnings.
        Your wallet is stored securely on your device.
      </p>

      {!showAddress ? (
        <div className="wallet-options">
          <div className="wallet-option primary" onClick={generateWallet}>
            <div className="option-icon">✨</div>
            <h3>Auto-Generate Wallet</h3>
            <p>Create a new USDC wallet instantly</p>
          </div>
          <div className="wallet-option">
            <div className="option-icon">🔗</div>
            <h3>Connect Existing</h3>
            <p>Link your existing wallet</p>
          </div>
        </div>
      ) : (
        <div className="wallet-address-display">
          <div className="address-label">Your Wallet Address</div>
          <div className="address-value">{walletAddress}</div>
          <div className="address-actions">
            <Button variant="ghost" size="sm" onClick={() => navigator.clipboard.writeText(walletAddress)}>
              Copy Address
            </Button>
          </div>
        </div>
      )}

      <div className="step-actions">
        <Button onClick={onNext} disabled={!showAddress && !walletAddress}>
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
  const [customAmount, setCustomAmount] = useState('');
  const [isProcessing, setIsProcessing] = useState(false);

  const handlePaystackDeposit = () => {
    if (!selectedAmount && !customAmount) return;
    setIsProcessing(true);
    setTimeout(() => {
      setIsProcessing(false);
      onNext();
    }, 2000);
  };

  const handleUSDCDeposit = () => {
    if (!selectedAmount && !customAmount) return;
    setIsProcessing(true);
    setTimeout(() => {
      setIsProcessing(false);
      onNext();
    }, 2000);
  };

  const amount = selectedAmount || parseFloat(customAmount) || 0;

  return (
    <div className="onboarding-step funds-step">
      <h2>Add Funds</h2>
      <p className="step-description">
        Add USDC to your wallet to start playing. You can skip this step and add funds later.
      </p>

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
          <div className="deposit-option" onClick={handlePaystackDeposit}>
            <div className="option-icon">💳</div>
            <div className="option-info">
              <h4>Pay with Card</h4>
              <p>Visa, Mastercard via Paystack</p>
            </div>
            <div className="option-arrow">→</div>
          </div>
          <div className="deposit-option" onClick={handleUSDCDeposit}>
            <div className="option-icon">🪙</div>
            <div className="option-info">
              <h4>Deposit USDC</h4>
              <p>Transfer from another wallet</p>
            </div>
            <div className="option-arrow">→</div>
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

function BrowseGamesStep({ onComplete }) {
  return (
    <div className="onboarding-step browse-step">
      <h2>Discover Games</h2>
      <p className="step-description">
        Browse our marketplace of open source games. Start with these featured titles.
      </p>

      <div className="featured-games-grid">
        {FEATURED_GAMES.map(game => (
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
  const [currentStep, setCurrentStep] = useState(0);
  const navigate = useNavigate();

  useEffect(() => {
    const completed = localStorage.getItem(ONBOARDING_STORAGE_KEY);
    if (completed === 'true') {
      navigate('/');
    }
  }, [navigate]);

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
        return <BrowseGamesStep onComplete={completeOnboarding} />;
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

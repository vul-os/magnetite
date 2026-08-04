import { useState } from 'react';
import './CookieConsent.css';

const CONSENT_KEY = 'cookie_consent';

export default function CookieConsent() {
  // Derive initial visibility from storage via a lazy initializer so we never
  // call setState inside an effect just to read a synchronous value on mount.
  const [isVisible, setIsVisible] = useState(() => {
    try {
      return !localStorage.getItem(CONSENT_KEY);
    } catch {
      return false;
    }
  });

  const handleAccept = () => {
    localStorage.setItem(CONSENT_KEY, 'accepted');
    setIsVisible(false);
  };

  const handleDecline = () => {
    localStorage.setItem(CONSENT_KEY, 'declined');
    setIsVisible(false);
  };

  if (!isVisible) return null;

  return (
    <div className="cookie-consent">
      <div className="cookie-consent-content">
        <p className="cookie-consent-message">
          We use cookies to enhance your experience. By continuing, you agree to our{' '}
          <a href="/privacy" className="cookie-consent-link">Privacy Policy</a>.
        </p>
        <div className="cookie-consent-actions">
          <button
            type="button"
            className="cookie-consent-btn cookie-consent-btn-decline"
            onClick={handleDecline}
          >
            Decline
          </button>
          <button
            type="button"
            className="cookie-consent-btn cookie-consent-btn-accept"
            onClick={handleAccept}
          >
            Accept
          </button>
        </div>
      </div>
    </div>
  );
}

import { useState, useEffect } from 'react';
import './CookieConsent.css';

const CONSENT_KEY = 'cookie_consent';

export default function CookieConsent() {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    const consent = localStorage.getItem(CONSENT_KEY);
    if (!consent) {
      setIsVisible(true);
    }
  }, []);

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

import { useState } from 'react';
import type { ChangeEvent } from 'react';

export interface TermsCheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  error?: string;
  termsHref?: string;
  privacyHref?: string;
  required?: boolean;
}

export default function TermsCheckbox({
  checked,
  onChange,
  error,
  termsHref = '/terms',
  privacyHref = '/privacy',
  required = true,
}: TermsCheckboxProps) {
  const [showError, setShowError] = useState(false);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    onChange(e.target.checked);
    if (e.target.checked) setShowError(false);
  };

  const handleBlur = () => {
    if (required && !checked) setShowError(true);
  };

  return (
    <div className="terms-checkbox-wrapper">
      <label className="terms-label">
        <input
          type="checkbox"
          checked={checked}
          onChange={handleChange}
          onBlur={handleBlur}
          className="terms-input"
        />
        <span className="terms-text">
          I agree to the{' '}
          <a href={termsHref} target="_blank" rel="noopener noreferrer">
            Terms of Service
          </a>{' '}
          and{' '}
          <a href={privacyHref} target="_blank" rel="noopener noreferrer">
            Privacy Policy
          </a>
        </span>
      </label>
      {(error || (showError && required && !checked)) && (
        <span className="input-error-text">
          {error || 'You must accept the terms to continue'}
        </span>
      )}
    </div>
  );
}
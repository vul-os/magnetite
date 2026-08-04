import { useState } from 'react';
import './Input.css';

const variantClasses = {
  default: 'input-default',
  error: 'input-error',
  success: 'input-success',
};

export default function Input({
  label,
  placeholder,
  error,
  helperText,
  type = 'text',
  id,
  className = '',
  isDisabled = false,
  isRequired = false,
  leftIcon,
  rightIcon,
  leftText,
  rightText,
  floatingLabel = false,
  size = 'md',
  ...props
}) {
  const [showPassword, setShowPassword] = useState(false);
  const [isFocused, setIsFocused] = useState(false);

  const inputId = id || label?.toLowerCase().replace(/\s+/g, '-');
  const errorId = inputId ? `${inputId}-error` : undefined;
  const helperId = inputId ? `${inputId}-helper` : undefined;

  const actualType = type === 'password' && showPassword ? 'text' : type;

  const isPasswordToggle = type === 'password';

  const wrapperClasses = [
    'input-wrapper',
    `input-${size}`,
    variantClasses[error ? 'error' : 'default'],
    floatingLabel ? 'input-floating' : '',
    isFocused ? 'input-focused' : '',
    isDisabled ? 'input-disabled' : '',
    (leftIcon || rightIcon || leftText || rightText) ? 'input-with-addons' : '',
    className,
  ].filter(Boolean).join(' ');

  const describedBy = [
    error && errorId,
    helperText && !error && helperId,
  ].filter(Boolean).join(' ') || undefined;

  return (
    <div className={wrapperClasses}>
      {label && !floatingLabel && (
        <label htmlFor={inputId} className="input-label">
          {label}
          {isRequired && (
            <span className="input-required" aria-hidden="true"> *</span>
          )}
        </label>
      )}

      <div className="input-container">
        {leftIcon && (
          <span className="input-addon input-addon-left" aria-hidden="true">{leftIcon}</span>
        )}
        {leftText && <span className="input-addon input-addon-left-text">{leftText}</span>}

        <div className="input-field-wrapper">
          {floatingLabel && (
            <label
              htmlFor={inputId}
              className={`input-floating-label ${placeholder || props.value ? 'input-floating-label-active' : ''}`}
            >
              {label}
              {isRequired && (
                <span className="input-required" aria-hidden="true"> *</span>
              )}
            </label>
          )}
          <input
            id={inputId}
            type={actualType}
            placeholder={floatingLabel ? ' ' : placeholder}
            className={`input-field ${error ? 'input-error' : ''}`}
            disabled={isDisabled}
            required={isRequired || undefined}
            aria-invalid={error ? 'true' : undefined}
            aria-required={isRequired || undefined}
            aria-describedby={describedBy}
            onFocus={() => setIsFocused(true)}
            onBlur={() => setIsFocused(false)}
            {...props}
          />
        </div>

        {isPasswordToggle && (
          <button
            type="button"
            className="input-password-toggle"
            onClick={() => setShowPassword(!showPassword)}
            aria-label={showPassword ? 'Hide password' : 'Show password'}
            aria-controls={inputId}
          >
            {showPassword ? (
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            ) : (
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
            )}
          </button>
        )}

        {rightIcon && !isPasswordToggle && (
          <span className="input-addon input-addon-right" aria-hidden="true">{rightIcon}</span>
        )}
        {rightText && <span className="input-addon input-addon-right-text">{rightText}</span>}
      </div>

      {error && (
        <span id={errorId} className="input-error-text" role="alert">
          {error}
        </span>
      )}
      {helperText && !error && (
        <span id={helperId} className="input-helper-text">
          {helperText}
        </span>
      )}
    </div>
  );
}

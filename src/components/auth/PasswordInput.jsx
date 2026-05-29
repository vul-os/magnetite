import { useState } from 'react';

const requirements = [
  { id: 'length', label: '8+ characters', test: (v) => v.length >= 8 },
  { id: 'upper', label: 'Uppercase letter', test: (v) => /[A-Z]/.test(v) },
  { id: 'number', label: 'Number', test: (v) => /\d/.test(v) },
  { id: 'symbol', label: 'Symbol', test: (v) => /[!@#$%^&*(),.?":{}|<>]/.test(v) },
];

function getStrength(password) {
  const passed = requirements.filter((r) => r.test(password)).length;
  if (passed === 0) return { level: 0, label: '', color: '' };
  if (passed <= 1) return { level: 1, label: 'Weak', color: 'var(--color-error)' };
  if (passed <= 3) return { level: 2, label: 'Medium', color: 'var(--color-warning)' };
  return { level: 3, label: 'Strong', color: 'var(--color-success)' };
}

export default function PasswordInput({
  value,
  onChange,
  showStrength = true,
  showRequirements = false,
  error,
  ...props
}) {
  const [show, setShow] = useState(false);
  const strength = getStrength(value || '');

  return (
    <div className="password-input-wrapper">
      <div className="password-input-container">
        <input
          type={show ? 'text' : 'password'}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className={`password-input ${error ? 'input-error' : ''}`}
          {...props}
        />
        <button
          type="button"
          className="password-toggle"
          onClick={() => setShow(!show)}
          aria-label={show ? 'Hide password' : 'Show password'}
        >
          {show ? '👁️' : '👁️‍🗨️'}
        </button>
      </div>

      {showStrength && value && (
        <div className="password-strength">
          <div className="strength-bars">
            {[1, 2, 3].map((level) => (
              <div
                key={level}
                className="strength-bar"
                style={{
                  backgroundColor: strength.level >= level ? strength.color : 'var(--color-border)',
                }}
              />
            ))}
          </div>
          <span className="strength-label" style={{ color: strength.color }}>
            {strength.label}
          </span>
        </div>
      )}

      {showRequirements && value && (
        <ul className="password-requirements">
          {requirements.map((req) => (
            <li
              key={req.id}
              className={`requirement ${req.test(value) ? 'passed' : ''}`}
            >
              {req.test(value) ? '✓' : '○'} {req.label}
            </li>
          ))}
        </ul>
      )}

      {error && <span className="input-error-text">{error}</span>}
    </div>
  );
}
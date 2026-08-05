import type { InputHTMLAttributes } from 'react';

export interface EmailInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'type'> {
  value: string;
  onChange: (value: string) => void;
  error?: string;
  placeholder?: string;
}

export default function EmailInput({
  value,
  onChange,
  error,
  placeholder = 'Email',
  ...props
}: EmailInputProps) {
  return (
    <div className="email-input-wrapper">
      <div className="email-input-container">
        <span className="email-icon">✉️</span>
        <input
          type="email"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={`email-input ${error ? 'input-error' : ''}`}
          {...props}
        />
      </div>
      {error && <span className="input-error-text">{error}</span>}
    </div>
  );
}
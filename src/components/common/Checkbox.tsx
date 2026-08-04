import type { ChangeEvent, InputHTMLAttributes } from 'react';
import './Checkbox.css';

export interface CheckboxProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'onChange'> {
  checked?: boolean;
  onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
  label?: string;
  disabled?: boolean;
  error?: string;
  name?: string;
  value?: string | number;
  className?: string;
}

export default function Checkbox({
  checked = false,
  onChange,
  label,
  disabled = false,
  error,
  name,
  value,
  className = '',
  ...props
}: CheckboxProps) {
  const checkboxId = label?.toLowerCase().replace(/\s+/g, '-');

  const wrapperClasses = [
    'checkbox-wrapper',
    disabled ? 'checkbox-disabled' : '',
    error ? 'checkbox-error' : '',
    className,
  ].filter(Boolean).join(' ');

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    if (onChange) {
      onChange(e);
    }
  };

  return (
    <div className={wrapperClasses}>
      <label htmlFor={checkboxId} className="checkbox-label">
        <input
          type="checkbox"
          id={checkboxId}
          name={name}
          value={value}
          checked={checked}
          onChange={handleChange}
          disabled={disabled}
          className="checkbox-input"
          {...props}
        />
        <span className="checkbox-custom">
          <svg
            className="checkbox-checkmark"
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        </span>
        {label && <span className="checkbox-text">{label}</span>}
      </label>
      {error && <span className="checkbox-error-text">{error}</span>}
    </div>
  );
}

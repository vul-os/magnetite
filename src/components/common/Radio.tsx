import type { InputHTMLAttributes, ReactNode, HTMLAttributes } from 'react';
import './Radio.css';

export interface RadioProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'value' | 'checked' | 'type'> {
  checked?: boolean;
  onChange?: (value?: string | number) => void;
  label?: ReactNode;
  disabled?: boolean;
  name?: string;
  value?: string | number;
  className?: string;
}

export default function Radio({
  checked = false,
  onChange,
  label,
  disabled = false,
  name,
  value,
  className = '',
  ...props
}: RadioProps) {
  const wrapperClasses = [
    'radio-wrapper',
    disabled ? 'radio-disabled' : '',
    className,
  ].filter(Boolean).join(' ');

  const handleChange = () => {
    if (!disabled && onChange) {
      onChange(value);
    }
  };

  return (
    <label className={wrapperClasses}>
      <input
        type="radio"
        name={name}
        value={value}
        checked={checked}
        onChange={handleChange}
        disabled={disabled}
        className="radio-input"
        {...props}
      />
      <span className={`radio-custom ${checked ? 'radio-checked' : ''}`}>
        <span className="radio-fill" />
      </span>
      {label && <span className="radio-label">{label}</span>}
    </label>
  );
}

export interface RadioGroupProps {
  children?: ReactNode;
  name?: string;
  value?: string | number;
  onChange?: (value?: string | number) => void;
  className?: string;
}

export function RadioGroup({
  children,
  name,
  value,
  onChange,
  className = '',
  ...props
}: RadioGroupProps) {
  const groupClasses = ['radio-group', className].filter(Boolean).join(' ');

  // Cast: `name`/`value`/`onChange` here are forwarded onto a <div>, which isn't a
  // valid target for them (pre-existing behavior, unrelated to this migration —
  // RadioGroup has no current callers). Preserved as-is; only the type is bridged.
  const divProps = {
    role: 'radiogroup',
    name,
    className: groupClasses,
    onChange,
    value,
    ...props,
  } as unknown as HTMLAttributes<HTMLDivElement>;

  return (
    <div {...divProps}>
      {children}
    </div>
  );
}

import type { ButtonHTMLAttributes, ReactNode } from 'react';
import './Switch.css';

type SwitchSize = 'sm' | 'md';

const sizeClasses: Record<SwitchSize, string> = {
  sm: 'sizeSm',
  md: 'sizeMd',
};

export interface SwitchProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onClick' | 'onChange' | 'type' | 'role'> {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  size?: SwitchSize;
  label?: ReactNode;
  id?: string;
  className?: string;
}

/**
 * Switch — accessible toggle button.
 *
 * Pass `label` to render a visible label alongside the switch, or
 * pass `aria-label` to provide a screen-reader-only label when the
 * switch appears without adjacent visible text.
 */
export default function Switch({
  checked = false,
  onChange,
  disabled = false,
  size = 'md',
  label,
  id,
  className = '',
  ...props
}: SwitchProps) {
  const classes = [
    'switch',
    sizeClasses[size],
    checked ? 'checked' : '',
    disabled ? 'disabled' : '',
    className,
  ].filter(Boolean).join(' ');

  const switchBtn = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      id={id}
      className={classes}
      disabled={disabled}
      onClick={() => onChange?.(!checked)}
      {...props}
    >
      <span className="switch-track" aria-hidden="true">
        <span className="switch-thumb" />
      </span>
    </button>
  );

  if (label) {
    return (
      <label className="switch-label-wrapper">
        {switchBtn}
        <span className="switch-label-text">{label}</span>
      </label>
    );
  }

  return switchBtn;
}

import './Radio.css';

export default function Radio({
  checked = false,
  onChange,
  label,
  disabled = false,
  name,
  value,
  className = '',
  ...props
}) {
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

export function RadioGroup({
  children,
  name,
  value,
  onChange,
  className = '',
  ...props
}) {
  const groupClasses = ['radio-group', className].filter(Boolean).join(' ');

  return (
    <div
      role="radiogroup"
      name={name}
      className={groupClasses}
      onChange={onChange}
      value={value}
      {...props}
    >
      {children}
    </div>
  );
}

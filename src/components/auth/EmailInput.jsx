export default function EmailInput({
  value,
  onChange,
  error,
  placeholder = 'Email',
  ...props
}) {
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
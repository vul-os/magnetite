import Spinner from '../common/Spinner';

export default function AuthForm({
  children,
  logo,
  title,
  subtitle,
  loading = false,
  showTerms = true,
}) {
  return (
    <div className="auth-card">
      {loading && (
        <div className="auth-overlay">
          <Spinner size="lg" />
        </div>
      )}

      <div className="auth-header">
        {logo && <div className="auth-logo">{logo}</div>}
        {title && <h1 className="auth-title">{title}</h1>}
        {subtitle && <p className="auth-subtitle">{subtitle}</p>}
      </div>

      <div className="auth-body">
        {children}
      </div>

      <div className="auth-footer">
        {showTerms && (
          <p className="auth-terms">
            By continuing, you agree to our{' '}
            <a href="/terms">Terms of Service</a> and{' '}
            <a href="/privacy">Privacy Policy</a>
          </p>
        )}
        <p className="auth-social-proof">
          Join thousands of teams using Magnetite securely every day
        </p>
      </div>
    </div>
  );
}

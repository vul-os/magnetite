import type { ReactNode } from 'react';
import Spinner from '../common/Spinner';

export interface AuthFormProps {
  children?: ReactNode;
  logo?: ReactNode;
  title?: string;
  subtitle?: string;
  loading?: boolean;
  showTerms?: boolean;
}

export default function AuthForm({
  children,
  logo,
  title,
  subtitle,
  loading = false,
  showTerms = true,
}: AuthFormProps) {
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

      {showTerms && (
        <div className="auth-footer">
          <p className="auth-terms">
            By continuing, you agree to our{' '}
            <a href="/terms">Terms of Service</a> and{' '}
            <a href="/privacy">Privacy Policy</a>
          </p>
        </div>
      )}
    </div>
  );
}

import { Component } from 'react';
import { Link } from 'react-router-dom';
import './ErrorBoundary.css';

class ErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false, error: null, errorId: null };
  }

  static getDerivedStateFromError(error) {
    return {
      hasError: true,
      error,
      errorId: `ERR-${Date.now().toString(36).toUpperCase()}`
    };
  }

  componentDidCatch(error, errorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null, errorId: null });
  };

  render() {
    if (this.state.hasError) {
      const { error, errorId } = this.state;
      return (
        <div className="error-boundary-fallback">
          <div className="error-boundary-content">
            <div className="error-boundary-icon">!</div>
            <h1>Something went wrong</h1>
            <p>An unexpected error occurred. Our team has been notified.</p>
            {errorId && (
              <div className="error-boundary-id">
                <span>Error ID: </span>
                <code>{errorId}</code>
              </div>
            )}
            <div className="error-boundary-actions">
              <button onClick={this.handleRetry} className="btn btn-primary">
                Try Again
              </button>
              <Link to="/" className="btn btn-secondary">
                Go Home
              </Link>
            </div>
            {process.env.NODE_ENV === 'development' && error && (
              <details className="error-boundary-details">
                <summary>Error Details</summary>
                <pre>{error.toString()}</pre>
              </details>
            )}
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;

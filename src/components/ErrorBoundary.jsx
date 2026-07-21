import { Component } from 'react';
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
              {/* A plain <a>, not <Link>: this boundary sits ABOVE
                  <BrowserRouter> in App.jsx precisely so it can catch a crash
                  anywhere, including inside the router tree itself. If the
                  router (or its context) is what crashed, <Link> would throw
                  again trying to read Router context that no longer exists —
                  masking the real error behind a second, more confusing one.
                  A full navigation is also the more honest recovery here: it
                  actually re-mounts the app instead of trusting client-side
                  routing state that may be the thing that broke. */}
              <a href="/" className="btn btn-secondary">
                Go Home
              </a>
            </div>
            {import.meta.env.DEV && error && (
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

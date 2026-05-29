import { useEffect, useState } from 'react';
import PropTypes from 'prop-types';
import './loading.css';

export default function LoadingPage({
  message = 'Loading...',
  progress,
  showProgressBar = false,
  animationType = 'pulse'
}) {
  const [fadeOut, setFadeOut] = useState(false);

  useEffect(() => {
    if (progress >= 100) {
      setFadeOut(true);
    }
  }, [progress]);

  return (
    <div className={`loading-page ${fadeOut ? 'loading-page-exit' : ''}`}>
      <div className="loading-page-content">
        <div className={`loading-logo-container ${animationType === 'spin' ? 'spin' : 'pulse'}`}>
          <div className="loading-logo">
            <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path
                d="M12 2L2 7L12 12L22 7L12 2Z"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M2 17L12 22L22 17"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M2 12L12 17L22 12"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </div>
          <div className="loading-logo-ring" />
          <div className="loading-logo-ring delay-1" />
        </div>

        <p className="loading-text">{message}</p>

        {showProgressBar && (
          <div className="loading-progress">
            <div className="loading-progress-bar">
              <div
                className="loading-progress-fill"
                style={{ width: `${Math.min(progress || 0, 100)}%` }}
              />
            </div>
            {progress !== undefined && (
              <span className="loading-progress-text">{Math.round(progress)}%</span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

LoadingPage.propTypes = {
  message: PropTypes.string,
  progress: PropTypes.number,
  showProgressBar: PropTypes.bool,
  animationType: PropTypes.oneOf(['pulse', 'spin'])
};

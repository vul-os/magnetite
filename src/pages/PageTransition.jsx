import { useEffect, useState } from 'react';
import PropTypes from 'prop-types';

export default function PageTransition({ children, isLoading = false, duration = 300 }) {
  const [isVisible, setIsVisible] = useState(!isLoading);

  useEffect(() => {
    if (isLoading) {
      const timer = setTimeout(() => setIsVisible(false), 0);
      return () => clearTimeout(timer);
    } else {
      const timer = setTimeout(() => setIsVisible(true), 50);
      return () => clearTimeout(timer);
    }
  }, [isLoading]);

  return (
    <div
      className={`page-transition ${isVisible ? 'page-transition-enter' : 'page-transition-exit'}`}
      style={{
        '--transition-duration': `${duration}ms`
      }}
    >
      {children}
    </div>
  );
}

PageTransition.propTypes = {
  children: PropTypes.node,
  isLoading: PropTypes.bool,
  duration: PropTypes.number
};

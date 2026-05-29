import { useState, useEffect, useRef } from 'react';
import './WidgetWrapper.css';

export default function WidgetWrapper({ children, icon, label }) {
  const [isOpen, setIsOpen] = useState(false);
  const [isAnimating, setIsAnimating] = useState(false);
  const [isVisible, setIsVisible] = useState(false);
  const widgetRef = useRef(null);

  useEffect(() => {
    if (isOpen) {
      setIsVisible(true);
      requestAnimationFrame(() => {
        setIsAnimating(true);
      });
    } else {
      setIsAnimating(false);
      const timer = setTimeout(() => {
        setIsVisible(false);
      }, 300);
      return () => clearTimeout(timer);
    }
  }, [isOpen]);

  useEffect(() => {
    const handleClickOutside = (event) => {
      if (widgetRef.current && !widgetRef.current.contains(event.target)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen]);

  const toggleWidget = () => setIsOpen(!isOpen);

  return (
    <div className="widget-container" ref={widgetRef}>
      {isVisible && (
        <div className={`widget-panel ${isAnimating ? 'widget-panel-open' : ''}`}>
          <div className="widget-header">
            <span className="widget-label">{label}</span>
            <button className="widget-close" onClick={toggleWidget} aria-label="Close">
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                <path d="M4 4l10 10M14 4l-10 10" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </button>
          </div>
          <div className="widget-content">
            {children}
          </div>
        </div>
      )}
      
      <button 
        className={`widget-toggle ${isOpen ? 'widget-toggle-active' : ''}`}
        onClick={toggleWidget}
        aria-label={isOpen ? 'Close widget' : 'Open widget'}
      >
        <span className={`widget-icon ${isOpen ? 'widget-icon-hidden' : ''}`}>
          {icon}
        </span>
        <span className={`widget-icon widget-icon-close ${!isOpen ? 'widget-icon-hidden' : ''}`}>
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <path d="M5 5l10 10M15 5l-10 10" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
          </svg>
        </span>
      </button>
    </div>
  );
}

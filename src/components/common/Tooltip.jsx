import { useState, useRef, useEffect, useId, useCallback } from 'react';
import { createPortal } from 'react-dom';
import './Tooltip.css';

const positionClasses = {
  top: 'tooltip-top',
  bottom: 'tooltip-bottom',
  left: 'tooltip-left',
  right: 'tooltip-right',
};

export default function Tooltip({
  content,
  position: initialPosition = 'top',
  children,
  delay = 200,
}) {
  const [isVisible, setIsVisible] = useState(false);
  const [position, setPosition] = useState(initialPosition);
  const [coords, setCoords] = useState({ top: 0, left: 0 });
  const triggerRef = useRef(null);
  const tooltipRef = useRef(null);
  const timeoutRef = useRef(null);
  const tooltipId = useId();

  const calculatePosition = useCallback((rect, tooltipRect) => {
    const viewportHeight = window.innerHeight;
    const viewportWidth = window.innerWidth;
    const margin = 8;
    let newPosition = initialPosition;

    if (initialPosition === 'top' && rect.top < tooltipRect.height + margin) {
      newPosition = 'bottom';
    } else if (initialPosition === 'bottom' && rect.bottom > viewportHeight - tooltipRect.height - margin) {
      newPosition = 'top';
    } else if (initialPosition === 'left' && rect.left < tooltipRect.width + margin) {
      newPosition = 'right';
    } else if (initialPosition === 'right' && rect.right > viewportWidth - tooltipRect.width - margin) {
      newPosition = 'left';
    }

    let top = 0;
    let left = 0;

    switch (newPosition) {
      case 'top':
        top = rect.top + window.scrollY - tooltipRect.height - margin;
        left = rect.left + window.scrollX + (rect.width - tooltipRect.width) / 2;
        break;
      case 'bottom':
        top = rect.bottom + window.scrollY + margin;
        left = rect.left + window.scrollX + (rect.width - tooltipRect.width) / 2;
        break;
      case 'left':
        top = rect.top + window.scrollY + (rect.height - tooltipRect.height) / 2;
        left = rect.left + window.scrollX - tooltipRect.width - margin;
        break;
      case 'right':
        top = rect.top + window.scrollY + (rect.height - tooltipRect.height) / 2;
        left = rect.right + window.scrollX + margin;
        break;
    }

    left = Math.max(margin, Math.min(left, viewportWidth - tooltipRect.width - margin));
    top = Math.max(margin, Math.min(top, viewportHeight - tooltipRect.height - margin));

    return { top, left, position: newPosition };
  }, [initialPosition]);

  const showTooltip = () => {
    timeoutRef.current = setTimeout(() => {
      if (triggerRef.current) {
        const rect = triggerRef.current.getBoundingClientRect();
        setCoords({ top: rect.top, left: rect.left, width: rect.width, height: rect.height });
        setPosition(initialPosition);
        setIsVisible(true);
      }
    }, delay);
  };

  const hideTooltip = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    setIsVisible(false);
  };

  useEffect(() => {
    if (isVisible && tooltipRef.current) {
      const tooltipRect = tooltipRef.current.getBoundingClientRect();
      const rect = {
        top: coords.top,
        left: coords.left,
        width: coords.width,
        height: coords.height,
        bottom: coords.top + coords.height,
        right: coords.left + coords.width,
      };
      const { top, left, position: finalPosition } = calculatePosition(rect, tooltipRect);
      setCoords((prev) => ({ ...prev, top, left }));
      setPosition(finalPosition);
    }
  }, [isVisible, coords.top, coords.left, coords.width, coords.height, calculatePosition]);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const tooltip = isVisible ? (
    <div
      ref={tooltipRef}
      id={tooltipId}
      className={`tooltip ${positionClasses[position]}`}
      style={{ top: coords.top, left: coords.left }}
      role="tooltip"
      aria-hidden={!isVisible}
    >
      <div className="tooltip-content">{content}</div>
      <div className="tooltip-arrow" />
    </div>
  ) : null;

  return (
    <>
      <span
        ref={triggerRef}
        className="tooltip-trigger"
        onMouseEnter={showTooltip}
        onMouseLeave={hideTooltip}
        onFocus={showTooltip}
        onBlur={hideTooltip}
        aria-describedby={isVisible ? tooltipId : undefined}
      >
        {children}
      </span>
      {createPortal(tooltip, document.body)}
    </>
  );
}
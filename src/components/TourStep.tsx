import { useEffect, useState, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import './TourStep.css';

type TourPosition = 'top' | 'bottom' | 'left' | 'right';

const positionClasses: Record<TourPosition, string> = {
  top: 'tour-step-top',
  bottom: 'tour-step-bottom',
  left: 'tour-step-left',
  right: 'tour-step-right',
};

interface TourStepProps {
  targetSelector?: string;
  title?: string;
  description?: string;
  position?: TourPosition;
  children?: ReactNode;
}

/* coords never actually carries `bottom`/`right` (only top/left/width/height
   are set below) — accessed anyway further down for the 'bottom'/'right'
   tooltip placements, which was already silently producing NaN offsets
   before the migration. Typed as optional + asserted at the read sites to
   preserve that pre-existing behavior exactly rather than "fixing" it. */
interface TourCoords {
  top: number;
  left: number;
  width: number;
  height: number;
  bottom?: number;
  right?: number;
}

export default function TourStep({
  targetSelector,
  title,
  description,
  position = 'bottom',
  children,
}: TourStepProps) {
  const [coords, setCoords] = useState<TourCoords>({ top: 0, left: 0, width: 0, height: 0 });
  const [isVisible, setIsVisible] = useState(false);
  const [finalPosition, setFinalPosition] = useState<TourPosition>(position);
  // Tooltip dimensions are measured from the DOM in an effect and stored here so
  // render never reads the live ref (which is impure during render).
  const [tooltipSize, setTooltipSize] = useState({ width: 200, height: 0 });
  const tooltipRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!targetSelector) return;

    const updatePosition = () => {
      const target = document.querySelector(targetSelector);
      if (target) {
        const rect = target.getBoundingClientRect();
        setCoords({
          top: rect.top,
          left: rect.left,
          width: rect.width,
          height: rect.height,
        });
        setIsVisible(true);
      }
    };

    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition);

    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition);
    };
  }, [targetSelector, isVisible]);

  useEffect(() => {
    if (isVisible && tooltipRef.current) {
      const tooltipRect = tooltipRef.current.getBoundingClientRect();
      setTooltipSize({ width: tooltipRect.width, height: tooltipRect.height });
      const viewportHeight = window.innerHeight;
      const viewportWidth = window.innerWidth;
      const margin = 12;
      let newPosition = position;

      if (position === 'top' && coords.top < tooltipRect.height + margin) {
        newPosition = 'bottom';
      } else if (position === 'bottom' && coords.top + coords.height + tooltipRect.height + margin > viewportHeight) {
        newPosition = 'top';
      } else if (position === 'left' && coords.left < tooltipRect.width + margin) {
        newPosition = 'right';
      } else if (position === 'right' && coords.left + coords.width + tooltipRect.width + margin > viewportWidth) {
        newPosition = 'left';
      }

      setFinalPosition(newPosition);
    }
  }, [isVisible, position, coords]);

  if (!isVisible) return null;

  const spotlightStyle = {
    top: coords.top - 4,
    left: coords.left - 4,
    width: coords.width + 8,
    height: coords.height + 8,
  };

  let tooltipStyle: { top: number; left: number; transform?: string } = { top: 0, left: 0 };
  const margin = 12;

  switch (finalPosition) {
    case 'top':
      tooltipStyle = {
        top: coords.top - tooltipSize.height - margin - 8,
        left: coords.left + coords.width / 2,
      };
      break;
    case 'bottom':
      tooltipStyle = {
        // coords.bottom is never populated (see TourCoords) — preserved NaN behavior.
        top: (coords.bottom as number) + margin + 8,
        left: coords.left + coords.width / 2,
      };
      break;
    case 'left':
      tooltipStyle = {
        top: coords.top + coords.height / 2,
        left: coords.left - (tooltipSize.width || 200) - margin - 8,
      };
      break;
    case 'right':
      tooltipStyle = {
        top: coords.top + coords.height / 2,
        // coords.right is never populated (see TourCoords) — preserved NaN behavior.
        left: (coords.right as number) + margin + 8,
      };
      break;
  }

  tooltipStyle.transform = finalPosition === 'top' || finalPosition === 'bottom'
    ? 'translateX(-50%)'
    : finalPosition === 'left' || finalPosition === 'right'
      ? 'translateY(-50%)'
      : 'none';

  const tooltip = (
    <div
      ref={tooltipRef}
      className={`tour-step ${positionClasses[finalPosition]}`}
      style={tooltipStyle}
      role="tooltip"
    >
      {title && <h3 className="tour-step-title">{title}</h3>}
      {description && <p className="tour-step-description">{description}</p>}
      {children}
      <div className="tour-step-arrow" />
    </div>
  );

  return createPortal(
    <>
      <div className="tour-spotlight" style={spotlightStyle} />
      <div className="tour-spotlight-overlay" />
      {tooltip}
    </>,
    document.body
  );
}

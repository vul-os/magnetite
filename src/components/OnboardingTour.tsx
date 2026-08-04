import { useEffect } from 'react';
import Button from './common/Button';
import TourStep from './TourStep';
import './OnboardingTour.css';

export default function OnboardingTour({
  steps,
  currentStep,
  isActive,
  isFirst,
  isLast,
  onNext,
  onBack,
  onSkip,
}) {
  useEffect(() => {
    if (isActive) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [isActive]);

  useEffect(() => {
    const handleKeyDown = (e) => {
      if (!isActive) return;
      if (e.key === 'Escape') onSkip();
      if (e.key === 'ArrowRight') onNext();
      if (e.key === 'ArrowLeft' && !isFirst) onBack();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isActive, isFirst, onNext, onBack, onSkip]);

  if (!isActive || !steps[currentStep]) return null;

  const step = steps[currentStep];

  return (
    <>
      <TourStep
        targetSelector={step.targetSelector}
        title={step.title}
        description={step.description}
        position={step.position}
      >
        <div className="tour-controls">
          <div className="tour-progress">
            {steps.map((_, index) => (
              <span
                key={index}
                className={`tour-progress-dot ${index === currentStep ? 'active' : ''} ${index < currentStep ? 'completed' : ''}`}
              />
            ))}
          </div>
          <div className="tour-buttons">
            {isFirst ? (
              <Button variant="ghost" size="sm" onClick={onSkip}>
                Skip
              </Button>
            ) : (
              <Button variant="ghost" size="sm" onClick={onBack}>
                Back
              </Button>
            )}
            <Button variant="primary" size="sm" onClick={onNext}>
              {isLast ? 'Finish' : 'Next'}
            </Button>
          </div>
        </div>
      </TourStep>
    </>
  );
}

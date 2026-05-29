import { useState, useCallback } from 'react';

export function useTour(initialSteps = [], options = {}) {
  const [currentStep, setCurrentStep] = useState(0);
  const [isActive, setIsActive] = useState(false);
  const [steps] = useState(initialSteps);

  const start = useCallback(() => {
    setCurrentStep(0);
    setIsActive(true);
  }, []);

  const stop = useCallback(() => {
    setIsActive(false);
    setCurrentStep(0);
  }, []);

  const next = useCallback(() => {
    setCurrentStep((prev) => {
      const maxStep = steps.length - 1;
      if (prev >= maxStep) {
        if (options.onComplete) options.onComplete();
        stop();
        return prev;
      }
      return prev + 1;
    });
  }, [steps.length, options.onComplete, stop]);

  const back = useCallback(() => {
    setCurrentStep((prev) => Math.max(0, prev - 1));
  }, []);

  const skip = useCallback(() => {
    if (options.onSkip) options.onSkip();
    stop();
  }, [options.onSkip, stop]);

  const goToStep = useCallback((stepIndex) => {
    if (stepIndex >= 0 && stepIndex < steps.length) {
      setCurrentStep(stepIndex);
    }
  }, [steps.length]);

  return {
    currentStep,
    totalSteps: steps.length,
    step: steps[currentStep],
    isActive,
    isFirst: currentStep === 0,
    isLast: currentStep === steps.length - 1,
    next,
    back,
    skip,
    start,
    stop,
    goToStep,
  };
}

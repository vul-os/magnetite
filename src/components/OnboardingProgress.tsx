import './OnboardingProgress.css';

export default function OnboardingProgress({ currentStep, totalSteps }) {
  return (
    <div className="onboarding-progress">
      <div className="progress-dots">
        {Array.from({ length: totalSteps }, (_, index) => (
          <div
            key={index}
            className={`progress-dot ${
              index < currentStep ? 'completed' : index === currentStep ? 'active' : ''
            }`}
          />
        ))}
      </div>
      <span className="progress-label">
        Step {currentStep + 1} of {totalSteps}
      </span>
    </div>
  );
}

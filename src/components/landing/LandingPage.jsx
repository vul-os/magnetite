import HeroSection from './HeroSection';
import FeaturesSection from './FeaturesSection';
import HowItWorksSection from './HowItWorksSection';
import DeveloperCTA from './DeveloperCTA';
import TestimonialsSection from './TestimonialsSection';
import FinalCTA from './FinalCTA';
import './Landing.css';

export default function LandingPage() {
  return (
    <div className="landing-page">
      <HeroSection />
      <FeaturesSection />
      <HowItWorksSection />
      <DeveloperCTA />
      <TestimonialsSection />
      <FinalCTA />
    </div>
  );
}

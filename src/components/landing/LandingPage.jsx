import HeroSection from './HeroSection';
import ProductShowcase from './ProductShowcase';
import FeaturesSection from './FeaturesSection';
import HowItWorksSection from './HowItWorksSection';
import DeveloperCTA from './DeveloperCTA';
import FinalCTA from './FinalCTA';
import './Landing.css';

export default function LandingPage() {
  return (
    <div className="landing-page">
      <HeroSection />
      <ProductShowcase />
      <FeaturesSection />
      <HowItWorksSection />
      <DeveloperCTA />
      <FinalCTA />
    </div>
  );
}

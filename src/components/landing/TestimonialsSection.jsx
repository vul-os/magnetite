import { useState } from 'react';
import { ChevronRightIcon, ChevronLeftIcon } from '../../assets/icons';
import './Landing.css';

const testimonials = [
  {
    quote:
      'Magnetite cut our infrastructure overhead to zero. We write Bevy, push to GitHub, and players are in-session within minutes. Revenue tripled in the first quarter.',
    name: 'Sarah Chen',
    role: 'Indie Developer',
    avatar: 'SC',
    tech: 'Bevy + WASM',
  },
  {
    quote:
      'The netcode primitives are production-grade out of the box. We shipped a 32-player battle royale on Magnetite in six weeks — something that would have taken six months building our own stack.',
    name: 'Marcus Rodriguez',
    role: 'Studio Lead, Pixel Forge',
    avatar: 'MR',
    tech: 'Multiplayer / 32-player',
  },
  {
    quote:
      'USDC payouts every week. No payment processors, no chargebacks, no waiting 60 days. It\'s the financial infrastructure I always wished existed for game developers.',
    name: 'Yuki Tanaka',
    role: 'Solo Developer',
    avatar: 'YT',
    tech: 'USDC Payouts',
  },
  {
    quote:
      'The Rust SDK is idiomatic, well-documented, and feels like it was written by game devs. Integrated leaderboards, matchmaking, and analytics in under a week.',
    name: 'Alex Kim',
    role: 'CTO, PlayChain',
    avatar: 'AK',
    tech: 'Rust SDK',
  },
  {
    quote:
      'From cargo new to production in 15 minutes. That\'s not an exaggeration. The DX is unmatched for a Rust game platform.',
    name: 'Emma Wilson',
    role: 'Rust Game Developer',
    avatar: 'EW',
    tech: 'Rust / Bevy',
  },
];

export default function TestimonialsSection() {
  const [currentIndex, setCurrentIndex] = useState(0);

  const next = () => {
    setCurrentIndex((prev) => (prev + 1) % testimonials.length);
  };

  const prev = () => {
    setCurrentIndex((prev) => (prev - 1 + testimonials.length) % testimonials.length);
  };

  const visibleTestimonials = [
    testimonials[currentIndex],
    testimonials[(currentIndex + 1) % testimonials.length],
    testimonials[(currentIndex + 2) % testimonials.length],
  ];

  return (
    <section className="testimonials-section" aria-labelledby="testimonials-heading">
      <div className="container">
        <div className="section-header-centered">
          <span className="kicker">// DEVELOPER STORIES</span>
          <h2 id="testimonials-heading" className="section-heading">
            Loved by{' '}
            <span className="gradient-text">Rust developers</span>
          </h2>
          <p className="section-lead">
            From solo game-jam entries to funded studios — here&apos;s what developers building on
            Magnetite have to say.
          </p>
        </div>

        <div className="testimonials-grid" aria-label="Developer testimonials">
          {visibleTestimonials.map((testimonial, index) => (
            <div key={index} className="testimonial-card">
              <div className="quote-mark" aria-hidden="true">&ldquo;</div>
              <p className="testimonial-quote">{testimonial.quote}</p>
              <div className="testimonial-footer">
                <div className="testimonial-author">
                  <div className="author-avatar" aria-hidden="true">{testimonial.avatar}</div>
                  <div className="author-info">
                    <div className="author-name">{testimonial.name}</div>
                    <div className="author-role">{testimonial.role}</div>
                  </div>
                </div>
                <span className="testimonial-tech-badge">{testimonial.tech}</span>
              </div>
            </div>
          ))}
        </div>

        <div className="testimonial-nav" role="group" aria-label="Testimonial navigation">
          <button onClick={prev} className="nav-btn" aria-label="Previous testimonial">
            <ChevronLeftIcon aria-hidden="true" />
          </button>
          <div className="testimonial-dots" role="tablist" aria-label="Testimonial indicators">
            {testimonials.map((_, index) => (
              <button
                key={index}
                role="tab"
                className={`dot ${index === currentIndex ? 'active' : ''}`}
                onClick={() => setCurrentIndex(index)}
                aria-label={`Go to testimonial ${index + 1}`}
                aria-selected={index === currentIndex}
              />
            ))}
          </div>
          <button onClick={next} className="nav-btn" aria-label="Next testimonial">
            <ChevronRightIcon aria-hidden="true" />
          </button>
        </div>
      </div>
    </section>
  );
}

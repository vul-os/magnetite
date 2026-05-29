import { useState } from 'react';
import { ChevronRightIcon, ChevronLeftIcon } from '../../assets/icons';
import { Card, CardBody } from '../common';
import './Landing.css';

const testimonials = [
  {
    quote: "Magnetite transformed how we launch HTML5 games. We went from managing servers to focusing entirely on game design. Our revenue tripled in three months.",
    name: "Sarah Chen",
    role: "Indie Developer",
    avatar: "SC",
  },
  {
    quote: "The edge network performance is incredible. Players in Asia and Europe both get sub-50ms latency. The matchmaking is seamless.",
    name: "Marcus Rodriguez",
    role: "Studio Lead, Pixel Forge",
    avatar: "MR",
  },
  {
    quote: "Getting paid in USDC was intimidating at first, but it's actually been great. Instant settlements with zero friction.",
    name: "Yuki Tanaka",
    role: "Solo Developer",
    avatar: "YT",
  },
  {
    quote: "The API is clean and well-documented. Integrated multiplayer functionality in under a week. Best infrastructure choice we've made.",
    name: "Alex Kim",
    role: "CTO, PlayChain",
    avatar: "AK",
  },
  {
    quote: "From zero to production in 15 minutes. That's not an exaggeration. The developer experience is unmatched.",
    name: "Emma Wilson",
    role: "HTML5 Game Developer",
    avatar: "EW",
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
    <section className="testimonials-section">
      <div className="container">
        <h2 className="section-title">
          Loved by <span className="gradient-text">Developers</span>
        </h2>
        <div className="testimonials-grid">
          {visibleTestimonials.map((testimonial, index) => (
            <Card key={index} variant="default" padding="lg">
              <CardBody>
                <div className="quote-mark">&ldquo;</div>
                <p className="testimonial-quote">{testimonial.quote}</p>
                <div className="testimonial-author">
                  <div className="author-avatar">{testimonial.avatar}</div>
                  <div className="author-info">
                    <div className="author-name">{testimonial.name}</div>
                    <div className="author-role">{testimonial.role}</div>
                  </div>
                </div>
              </CardBody>
            </Card>
          ))}
        </div>
        <div className="testimonial-nav">
          <button onClick={prev} className="nav-btn" aria-label="Previous">
            <ChevronLeftIcon />
          </button>
          <div className="testimonial-dots">
            {testimonials.map((_, index) => (
              <button
                key={index}
                className={`dot ${index === currentIndex ? 'active' : ''}`}
                onClick={() => setCurrentIndex(index)}
                aria-label={`Go to testimonial ${index + 1}`}
              />
            ))}
          </div>
          <button onClick={next} className="nav-btn" aria-label="Next">
            <ChevronRightIcon />
          </button>
        </div>
      </div>
    </section>
  );
}

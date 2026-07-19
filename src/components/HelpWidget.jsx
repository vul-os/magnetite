import { useState } from 'react';
import './HelpWidget.css';

const faqs = [
  {
    question: 'How do I get started?',
    answer: 'Create an account, complete your profile, and browse the Magnetite marketplace to discover Rust games compiled to WebAssembly — playable right in your browser.',
  },
  {
    question: 'How do payments work?',
    answer: 'Non-custodially. You link a wallet you control and pay the developer or operator directly in USDC; the rail returns a signed receipt that unlocks what you bought. We never hold a balance, so there is nothing to deposit and nothing to withdraw.',
  },
  {
    question: 'How can I become a game developer?',
    answer: 'Head to the Developer Dashboard, set up your developer account, and publish Rust games using our SDK. We handle hosting and matchmaking; buyers pay your wallet directly, so there are no payouts to handle.',
  },
  {
    question: 'What games are available?',
    answer: 'Rust games of every scale — from weekend game-jam arcades to large-scale multiplayer titles, all compiled to WebAssembly for instant in-browser play or native download.',
  },
  {
    question: 'How do I contact support?',
    answer: 'Email us at support@magnetite.io or use the contact form. Our team is available around the clock.',
  },
];

export default function HelpWidget() {
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedFaq, setExpandedFaq] = useState(null);

  const filteredFaqs = faqs.filter(faq =>
    faq.question.toLowerCase().includes(searchQuery.toLowerCase()) ||
    faq.answer.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const toggleFaq = (index) => {
    setExpandedFaq(prev => (prev === index ? null : index));
  };

  return (
    <div className="help-widget">
      <div className="help-search">
        <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
          <circle cx="11" cy="11" r="8"/>
          <path d="M21 21l-4.35-4.35"/>
        </svg>
        <input
          type="text"
          placeholder="Search help articles..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="search-input"
          aria-label="Search help articles"
        />
      </div>

      <div className="help-section">
        <h4 className="help-section-title">// FAQ</h4>
        <div className="faq-list" role="list">
          {filteredFaqs.length > 0 ? (
            filteredFaqs.map((faq, index) => (
              <div key={index} className="faq-item" role="listitem">
                <button
                  className={`faq-question ${expandedFaq === index ? 'faq-expanded' : ''}`}
                  onClick={() => toggleFaq(index)}
                  aria-expanded={expandedFaq === index}
                  aria-controls={`faq-answer-${index}`}
                >
                  <span>{faq.question}</span>
                  <svg className="faq-chevron" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </button>
                {expandedFaq === index && (
                  <div id={`faq-answer-${index}`} className="faq-answer">
                    <p>{faq.answer}</p>
                  </div>
                )}
              </div>
            ))
          ) : (
            <p className="no-results">No matching articles found.</p>
          )}
        </div>
      </div>

      <div className="help-contact">
        <h4 className="help-section-title">// SUPPORT</h4>
        <p className="contact-description">
          Our team is available 24/7. Reach out any time.
        </p>
        <a href="mailto:support@magnetite.io" className="contact-button">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
            <polyline points="22,6 12,13 2,6"/>
          </svg>
          Email Support
        </a>
      </div>
    </div>
  );
}

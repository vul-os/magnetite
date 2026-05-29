import { useState } from 'react';
import './HelpWidget.css';

const faqs = [
  {
    question: 'How do I get started?',
    answer: 'Create an account, complete your profile, and browse our game marketplace to start playing.',
  },
  {
    question: 'How do payments work?',
    answer: 'You can deposit funds using various payment methods. Earnings from games are credited to your wallet automatically.',
  },
  {
    question: 'How can I become a developer?',
    answer: 'Visit our Developer Dashboard to set up your developer account and start publishing your games.',
  },
  {
    question: 'What games are available?',
    answer: 'We offer a variety of browser-based games across multiple categories including puzzles, strategy, and arcade.',
  },
  {
    question: 'How do I contact support?',
    answer: 'Use the contact form below or email us at support@magnetite.gg for assistance.',
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
    setExpandedFaq(expandedFaq === index ? null : index);
  };

  return (
    <div className="help-widget">
      <div className="help-search">
        <svg className="search-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="11" cy="11" r="8"/>
          <path d="M21 21l-4.35-4.35"/>
        </svg>
        <input
          type="text"
          placeholder="Search help articles..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="search-input"
        />
      </div>

      <div className="help-section">
        <h4 className="help-section-title">Frequently Asked Questions</h4>
        <div className="faq-list">
          {filteredFaqs.length > 0 ? (
            filteredFaqs.map((faq, index) => (
              <div key={index} className="faq-item">
                <button
                  className={`faq-question ${expandedFaq === index ? 'faq-expanded' : ''}`}
                  onClick={() => toggleFaq(index)}
                  aria-expanded={expandedFaq === index}
                >
                  <span>{faq.question}</span>
                  <svg className="faq-chevron" width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </button>
                {expandedFaq === index && (
                  <div className="faq-answer">
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
        <h4 className="help-section-title">Still need help?</h4>
        <p className="contact-description">
          Our support team is available 24/7 to assist you with any questions.
        </p>
        <a href="mailto:support@magnetite.gg" className="contact-button">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
            <polyline points="22,6 12,13 2,6"/>
          </svg>
          Contact Support
        </a>
      </div>
    </div>
  );
}

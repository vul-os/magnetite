import { useState, useMemo } from 'react';
import Layout from '../components/Layout';
import { faqData, contactInfo } from '../data/faqData';

export default function FAQ() {
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedCategories, setExpandedCategories] = useState(
    faqData.reduce((acc, cat) => ({ ...acc, [cat.category]: true }), {})
  );
  const [expandedQuestions, setExpandedQuestions] = useState({});

  const toggleCategory = (category) => {
    setExpandedCategories(prev => ({
      ...prev,
      [category]: !prev[category]
    }));
  };

  const toggleQuestion = (category, index) => {
    const key = `${category}-${index}`;
    setExpandedQuestions(prev => ({
      ...prev,
      [key]: !prev[key]
    }));
  };

  const filteredData = useMemo(() => {
    if (!searchQuery.trim()) return faqData;

    const query = searchQuery.toLowerCase();
    return faqData
      .map(category => ({
        ...category,
        questions: category.questions.filter(
          q => q.q.toLowerCase().includes(query) || q.a.toLowerCase().includes(query)
        )
      }))
      .filter(category => category.questions.length > 0);
  }, [searchQuery]);

  const totalQuestions = filteredData.reduce((sum, cat) => sum + cat.questions.length, 0);

  return (
    <Layout>
      <div className="faq-page">
        <header className="faq-header">
          <h1>Frequently Asked Questions</h1>
          <p>Find answers to common questions about Magnetite</p>
        </header>

        <div className="faq-search">
          <svg className="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" />
          </svg>
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search questions..."
            className="faq-search-input"
          />
          {searchQuery && (
            <button
              className="search-clear"
              onClick={() => setSearchQuery('')}
              aria-label="Clear search"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>

        {searchQuery && (
          <p className="search-results-count">
            Found {totalQuestions} question{totalQuestions !== 1 ? 's' : ''}
          </p>
        )}

        <div className="faq-categories">
          {filteredData.map((category) => (
            <div key={category.category} className="faq-category">
              <button
                className="category-header"
                onClick={() => toggleCategory(category.category)}
                aria-expanded={expandedCategories[category.category]}
              >
                <div className="category-title">
                  <span className="category-icon">{category.icon}</span>
                  <h2>{category.category}</h2>
                  <span className="question-count">{category.questions.length} questions</span>
                </div>
                <svg
                  className={`chevron ${expandedCategories[category.category] ? 'expanded' : ''}`}
                  width="20"
                  height="20"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path d="M6 9l6 6 6-6" />
                </svg>
              </button>

              {expandedCategories[category.category] && (
                <div className="category-questions">
                  {category.questions.map((item, index) => {
                    const key = `${category.category}-${index}`;
                    const isExpanded = expandedQuestions[key];
                    return (
                      <div key={key} className={`faq-item ${isExpanded ? 'expanded' : ''}`}>
                        <button
                          className="faq-question"
                          onClick={() => toggleQuestion(category.category, index)}
                          aria-expanded={isExpanded}
                        >
                          <span>{item.q}</span>
                          <svg
                            className="chevron"
                            width="16"
                            height="16"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                          >
                            <path d="M6 9l6 6 6-6" />
                          </svg>
                        </button>
                        {isExpanded && (
                          <div className="faq-answer">
                            <p>{item.a}</p>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))}
        </div>

        {filteredData.length === 0 && (
          <div className="faq-empty">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <circle cx="11" cy="11" r="8" />
              <path d="M21 21l-4.35-4.35" />
              <path d="M8 8l6 6M14 8l-6 6" />
            </svg>
            <h3>No results found</h3>
            <p>Try adjusting your search terms or browse all categories</p>
            <button className="btn btn-secondary" onClick={() => setSearchQuery('')}>
              Clear Search
            </button>
          </div>
        )}

        <div className="faq-contact">
          <h3>Still have questions?</h3>
          <p>Can't find what you're looking for? Reach out to our support team.</p>
          <div className="contact-options">
            <a href={`mailto:${contactInfo.email}`} className="contact-link">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="2" y="4" width="20" height="16" rx="2" />
                <path d="M22 7l-10 6L2 7" />
              </svg>
              {contactInfo.email}
            </a>
            <a href={`https://${contactInfo.discord}`} target="_blank" rel="noopener noreferrer" className="contact-link">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
              </svg>
              {contactInfo.discord}
            </a>
            <a href={`https://twitter.com/${contactInfo.twitter}`} target="_blank" rel="noopener noreferrer" className="contact-link">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
              </svg>
              {contactInfo.twitter}
            </a>
          </div>
        </div>
      </div>
    </Layout>
  );
}

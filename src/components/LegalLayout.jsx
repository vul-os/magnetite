import Layout from './Layout';

export default function LegalLayout({ title, lastUpdated, sections, children }) {
  const handlePrint = () => {
    window.print();
  };

  return (
    <Layout>
      <div className="legal-page">
        <div className="legal-container">
          <aside className="legal-sidebar">
            <nav className="legal-toc">
              <h4>Table of Contents</h4>
              <ul>
                {sections.map((section) => (
                  <li key={section.id}>
                    <a href={`#${section.id}`}>{section.title}</a>
                  </li>
                ))}
              </ul>
            </nav>
            <div className="legal-sidebar-footer">
              <p className="last-updated">Last updated: {lastUpdated}</p>
              <button onClick={handlePrint} className="btn btn-secondary print-btn">
                Print Page
              </button>
            </div>
          </aside>

          <main className="legal-content">
            <header className="legal-header">
              <h1>{title}</h1>
            </header>
            <div className="legal-body">
              {children}
            </div>
          </main>
        </div>
      </div>
    </Layout>
  );
}

export function LegalSection({ id, title, children }) {
  return (
    <section id={id} className="legal-section">
      <h2>{title}</h2>
      <div className="legal-section-content">
        {children}
      </div>
    </section>
  );
}

import { useEffect, useState, useRef } from 'react';
import type { MouseEvent, ReactNode } from 'react';
import Layout from './Layout';
import './LegalLayout.css';

export interface LegalSectionInfo {
  id: string;
  title: string;
}

export interface LegalLayoutProps {
  title?: string;
  lastUpdated?: string;
  sections: LegalSectionInfo[];
  children?: ReactNode;
}

/**
 * LegalLayout — sticky in-page nav with active-section highlighting.
 * Owned by data-page-completeness partition.
 */
export default function LegalLayout({ title, lastUpdated, sections, children }: LegalLayoutProps) {
  const [activeId, setActiveId] = useState<string | null>(sections[0]?.id ?? null);
  const observerRef = useRef<IntersectionObserver | null>(null);

  /* Track which section is currently visible via IntersectionObserver */
  useEffect(() => {
    if (!sections || sections.length === 0) return;

    const headings = sections
      .map(s => document.getElementById(s.id))
      .filter((el): el is HTMLElement => el !== null);

    if (headings.length === 0) return;

    observerRef.current = new IntersectionObserver(
      (entries) => {
        // Pick the first intersecting entry (topmost in document order)
        const intersecting = entries
          .filter(e => e.isIntersecting)
          .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);

        if (intersecting.length > 0) {
          setActiveId(intersecting[0].target.id);
        }
      },
      {
        rootMargin: '-10% 0px -70% 0px',
        threshold: 0,
      }
    );

    headings.forEach(el => observerRef.current?.observe(el));

    return () => observerRef.current?.disconnect();
  }, [sections]);

  const handlePrint = () => window.print();

  const handleTocClick = (e: MouseEvent<HTMLAnchorElement>, id: string) => {
    e.preventDefault();
    const target = document.getElementById(id);
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      /* Move focus to the section heading for keyboard users */
      const heading = target.querySelector('h2');
      if (heading) {
        heading.setAttribute('tabindex', '-1');
        heading.focus({ preventScroll: true });
      }
      setActiveId(id);
    }
  };

  return (
    <Layout>
      <div className="legal-page reveal">
        <div className="legal-container">
          {/* ── Sticky sidebar ── */}
          <aside className="legal-sidebar" aria-label="Table of contents">
            <nav className="legal-toc">
              <h4>Contents</h4>
              <ul>
                {sections.map((section) => (
                  <li key={section.id}>
                    <a
                      href={`#${section.id}`}
                      className={activeId === section.id ? 'toc-active' : ''}
                      onClick={(e) => handleTocClick(e, section.id)}
                      aria-current={activeId === section.id ? 'location' : undefined}
                    >
                      {section.title}
                    </a>
                  </li>
                ))}
              </ul>
            </nav>

            <div className="legal-sidebar-footer">
              <p className="last-updated">Updated: {lastUpdated}</p>
              <button
                onClick={handlePrint}
                className="btn btn-secondary btn-sm print-btn"
                aria-label="Print this page"
              >
                Print Page
              </button>
            </div>
          </aside>

          {/* ── Main content ── */}
          <main className="legal-content" id="legal-main">
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

export interface LegalSectionProps {
  id: string;
  title: string;
  children?: ReactNode;
}

export function LegalSection({ id, title, children }: LegalSectionProps) {
  return (
    <section id={id} className="legal-section">
      <h2>{title}</h2>
      <div className="legal-section-content">
        {children}
      </div>
    </section>
  );
}

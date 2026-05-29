import { Link } from 'react-router-dom';
import { GithubIcon, TwitterIcon, DiscordIcon } from '../assets/icons';

export default function Footer() {
  const currentYear = new Date().getFullYear();

  const linkColumns = [
    {
      title: 'Platform',
      links: [
        { label: 'Marketplace', path: '/marketplace' },
        { label: 'Developers', path: '/developers' },
        { label: 'Pricing', path: '/pricing' },
        { label: 'FAQ', path: '/faq' },
      ],
    },
    {
      title: 'Developers',
      links: [
        { label: 'SDK', path: '/developers/sdk' },
        { label: 'Documentation', path: '/docs' },
        { label: 'GitHub', path: 'https://github.com/anomalyco/magnetite', external: true },
        { label: 'Discord', path: 'https://discord.gg/magnetite', external: true },
      ],
    },
    {
      title: 'Company',
      links: [
        { label: 'About', path: '/about' },
        { label: 'Careers', path: '/careers' },
        { label: 'Contact', path: '/contact' },
        { label: 'Blog', path: '/blog' },
      ],
    },
    {
      title: 'Legal',
      links: [
        { label: 'Terms', path: '/terms' },
        { label: 'Privacy', path: '/privacy' },
        { label: 'Cookies', path: '/cookies' },
      ],
    },
  ];

  return (
    <footer className="footer">
      <div className="footer-container">
        <div className="footer-top">
          <div className="footer-brand">
            <Link to="/" className="footer-logo">
              <div className="logo-icon">M</div>
              <span className="logo-text">Magnetite</span>
            </Link>
            <p className="footer-tagline">
              Open source games. Real money. No middlemen.
            </p>
            <div className="footer-social">
              <a
                href="https://discord.gg/magnetite"
                target="_blank"
                rel="noopener noreferrer"
                className="social-link"
                aria-label="Discord"
              >
                <DiscordIcon />
              </a>
              <a
                href="https://twitter.com/magnetite"
                target="_blank"
                rel="noopener noreferrer"
                className="social-link"
                aria-label="Twitter"
              >
                <TwitterIcon />
              </a>
              <a
                href="https://github.com/anomalyco/magnetite"
                target="_blank"
                rel="noopener noreferrer"
                className="social-link"
                aria-label="GitHub"
              >
                <GithubIcon />
              </a>
            </div>
          </div>

          <div className="footer-links">
            {linkColumns.map((column) => (
              <div key={column.title} className="footer-column">
                <h4 className="footer-column-title">{column.title}</h4>
                <ul className="footer-column-links">
                  {column.links.map((link) => (
                    <li key={link.label}>
                      {link.external || link.path.startsWith('http') ? (
                        <a
                          href={link.path}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="footer-link"
                        >
                          {link.label}
                        </a>
                      ) : (
                        <Link to={link.path} className="footer-link">
                          {link.label}
                        </Link>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        <div className="footer-bottom">
          <div className="footer-copyright">
            <p>&copy; {currentYear} Magnetite. All rights reserved.</p>
          </div>
          <div className="footer-badge">
            <span className="badge-text">Built with</span>
            <svg className="rust-icon" viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
              <path d="M23.9 11.2c-.4-1.2-1.9-1.5-3.1-.8l-2.9 1.7c-.2.1-.3.2-.5.1l-2.9-1.7c-.6-.3-1.3-.3-1.9 0L10.1 12l-2.8 1.7c-.2.1-.3.1-.5 0L3.9 12c-.6-.4-1.3-.4-1.9 0L.1 13.7c-1.2.7-2.7.5-3.1-.7-.4-1.2.2-2.5 1.3-3l2.9-1.7c.2-.1.3-.2.5-.1l2.9 1.7c.6.4 1.3.4 1.9 0L10.1 8l2.8-1.7c.2-.1.3-.1.5 0l2.9 1.7c.6.3 1.3.3 1.9 0l2.9-1.7c.2-.1.3 0 .5.1l2.9 1.7c1.1.5 1.7 1.8 1.3 3z"/>
              <path d="M11.5 15.5c-.5.3-1.1.3-1.6 0l-4-2.5c-.4-.3-.5-.9-.2-1.3.3-.4.9-.5 1.3-.2l4 2.5c.4.3.5.9.2 1.3-.2.1-.4.2-.7.2z"/>
            </svg>
            <span className="badge-text">Rust</span>
          </div>
        </div>
      </div>
    </footer>
  );
}

import { Link } from 'react-router-dom';
import { HomeIcon, ChevronRightIcon } from '../assets/icons';

export default function Breadcrumb({ items }) {
  if (!items || items.length === 0) return null;

  return (
    <nav className="breadcrumb" aria-label="Breadcrumb">
      <ol className="breadcrumb-list">
        <li className="breadcrumb-item">
          <Link to="/" className="breadcrumb-link">
            <HomeIcon className="breadcrumb-home-icon" />
            <span>Home</span>
          </Link>
        </li>
        {items.map((item, index) => {
          const isLast = index === items.length - 1;
          return (
            <li key={item.path || index} className="breadcrumb-item">
              <ChevronRightIcon className="breadcrumb-separator" />
              {isLast ? (
                <span className="breadcrumb-current">{item.label}</span>
              ) : (
                <Link to={item.path} className="breadcrumb-link">
                  {item.label}
                </Link>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

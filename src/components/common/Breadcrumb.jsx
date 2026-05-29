import { Link } from 'react-router-dom';
import { ChevronRightIcon, HomeIcon } from '../../assets/icons';

export default function Breadcrumb({ items = [], separator: Separator = ChevronRightIcon }) {
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
            <li key={item.href || index} className="breadcrumb-item">
              <Separator className="breadcrumb-separator" />
              {isLast || !item.href ? (
                <span className="breadcrumb-current">{item.label}</span>
              ) : (
                <Link to={item.href} className="breadcrumb-link">
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

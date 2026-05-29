import { Link, useLocation } from 'react-router-dom';

const NAV_ITEMS = [
  { id: 'dashboard', label: 'Dashboard', icon: '◉', path: '/admin' },
  { id: 'users', label: 'Users', icon: '👥', path: '/admin/users' },
  { id: 'games', label: 'Games', icon: '🎮', path: '/admin/games' },
  { id: 'finance', label: 'Finance', icon: '💰', path: '/admin/finance' },
  { id: 'settings', label: 'Settings', icon: '⚙️', path: '/admin/settings' },
];

export default function AdminSidebar() {
  const location = useLocation();

  const isActive = (item) => {
    if (item.id === 'dashboard') {
      return location.pathname === '/admin' || location.pathname === '/admin/';
    }
    return location.pathname.startsWith(item.path);
  };

  return (
    <aside className="admin-sidebar">
      <div className="sidebar-header">
        <Link to="/admin" className="admin-logo">
          <span className="logo-icon">⚡</span>
          <span className="logo-text">Admin</span>
        </Link>
      </div>
      <nav className="sidebar-nav">
        {NAV_ITEMS.map(item => (
          <Link
            key={item.id}
            to={item.path}
            className={`sidebar-item ${isActive(item) ? 'active' : ''}`}
          >
            <span className="sidebar-icon">{item.icon}</span>
            <span>{item.label}</span>
          </Link>
        ))}
      </nav>
      <div className="sidebar-footer">
        <Link to="/" className="back-to-site">
          <span>←</span>
          <span>Back to Site</span>
        </Link>
      </div>
    </aside>
  );
}

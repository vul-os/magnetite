import { Navigate } from 'react-router-dom';
import { useAuth } from '../../hooks/useAuth';

export function AdminRoute({ children }) {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return null;
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  if (user.role !== 'admin' && user.isAdmin !== true) {
    return <Navigate to="/403" replace />;
  }

  return children;
}

export default AdminRoute;

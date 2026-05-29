import { createContext, useContext, useState, useEffect } from 'react';

const AuthContext = createContext();

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null);
  const [token, setToken] = useState(localStorage.getItem('token'));
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const validateToken = async () => {
      const storedToken = localStorage.getItem('token');
      if (storedToken) {
        setToken(storedToken);
        const mockUser = { id: 1, email: 'user@example.com', name: 'Demo User' };
        setUser(mockUser);
      }
      setIsLoading(false);
    };
    validateToken();
  }, []);

  const login = async (email, _password) => {
    const mockToken = 'mock-jwt-token-' + Date.now();
    const mockUser = { id: 1, email, name: email.split('@')[0] };
    localStorage.setItem('token', mockToken);
    setToken(mockToken);
    setUser(mockUser);
    return { success: true };
  };

  const register = async (email, _password, name) => {
    const mockToken = 'mock-jwt-token-' + Date.now();
    const mockUser = { id: 1, email, name };
    localStorage.setItem('token', mockToken);
    setToken(mockToken);
    setUser(mockUser);
    return { success: true };
  };

  const logout = () => {
    localStorage.removeItem('token');
    setToken(null);
    setUser(null);
  };

  const isAuthenticated = !!token;

  return (
    <AuthContext.Provider value={{ user, token, login, register, logout, isAuthenticated, isLoading }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) throw new Error('useAuth must be used within AuthProvider');
  return context;
}

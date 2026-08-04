import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Login from '../pages/Login';

const mockLogin = vi.fn();

vi.mock('../hooks/useAuth', () => ({
  useAuth: () => ({
    login: mockLogin,
  }),
}));

vi.mock('../api/client', () => ({
  getOAuthUrl: () => 'http://mock.auth/provider',
}));

describe('Login', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders login form', () => {
    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /welcome back/i })).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/email/i)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/password/i)).toBeInTheDocument();
  });

  it('has OAuth buttons present', () => {
    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>
    );
    expect(screen.getByRole('button', { name: /continue with google/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /continue with discord/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /continue with github/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /continue with gitlab/i })).toBeInTheDocument();
  });

  it('handles form submission', async () => {
    mockLogin.mockResolvedValue({});

    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByPlaceholderText(/email/i), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByPlaceholderText(/password/i), { target: { value: 'password123' } });
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    await waitFor(() => {
      expect(mockLogin).toHaveBeenCalledWith('test@example.com', 'password123');
    });
  });

  it('shows error message on failed login', async () => {
    mockLogin.mockRejectedValue(new Error('Invalid credentials'));

    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>
    );

    fireEvent.change(screen.getByPlaceholderText(/email/i), { target: { value: 'bad@example.com' } });
    fireEvent.change(screen.getByPlaceholderText(/password/i), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    await waitFor(() => {
      expect(screen.getByText('Invalid credentials')).toBeInTheDocument();
    });
  });
});
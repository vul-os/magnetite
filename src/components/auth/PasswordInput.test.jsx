import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import PasswordInput from './PasswordInput';

describe('PasswordInput', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders password input', () => {
    render(<PasswordInput value="" onChange={vi.fn()} />);
    const input = document.querySelector('input[type="password"]');
    expect(input).toBeInTheDocument();
  });

  it('show/hide toggle works', () => {
    render(<PasswordInput value="secret" onChange={vi.fn()} />);
    const input = document.querySelector('input');
    expect(input.type).toBe('password');

    fireEvent.click(screen.getByRole('button', { name: /show password/i }));
    expect(input.type).toBe('text');

    fireEvent.click(screen.getByRole('button', { name: /hide password/i }));
    expect(input.type).toBe('password');
  });

  it('strength indicator updates on password change', async () => {
    const { rerender } = render(<PasswordInput value="" onChange={vi.fn()} showStrength />);
    expect(screen.queryByText('Weak')).not.toBeInTheDocument();

    rerender(<PasswordInput value="password" onChange={vi.fn()} showStrength />);
    await waitFor(() => {
      expect(screen.getByText('Weak')).toBeInTheDocument();
    });

    rerender(<PasswordInput value="Password1" onChange={vi.fn()} showStrength />);
    await waitFor(() => {
      expect(screen.getByText('Medium')).toBeInTheDocument();
    });

    rerender(<PasswordInput value="StrongPass1!" onChange={vi.fn()} showStrength />);
    await waitFor(() => {
      expect(screen.getByText('Strong')).toBeInTheDocument();
    });
  });

  it('handles onChange', () => {
    const handleChange = vi.fn();
    render(<PasswordInput value="" onChange={handleChange} />);
    fireEvent.change(document.querySelector('input'), { target: { value: 'newpassword' } });
    expect(handleChange).toHaveBeenCalledWith('newpassword');
  });
});

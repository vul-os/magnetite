import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import Input from './Input';

describe('Input', () => {
  it('renders with label', () => {
    render(<Input label="Email" />);
    expect(screen.getByLabelText('Email')).toBeInTheDocument();
  });

  it('shows error state', () => {
    render(<Input label="Email" error="Invalid email" />);
    expect(screen.getByText('Invalid email')).toBeInTheDocument();
    expect(screen.getByLabelText('Email')).toHaveClass('input-error');
  });

  it('handles onChange', () => {
    const handleChange = vi.fn();
    render(<Input label="Email" onChange={handleChange} />);
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'test@example.com' } });
    expect(handleChange).toHaveBeenCalled();
  });

  it('renders without label', () => {
    render(<Input placeholder="Enter text" />);
    expect(screen.getByPlaceholderText('Enter text')).toBeInTheDocument();
  });

  it('shows helper text when no error', () => {
    render(<Input label="Email" helperText="Enter your email address" />);
    expect(screen.getByText('Enter your email address')).toBeInTheDocument();
  });
});

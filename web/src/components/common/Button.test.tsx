import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import Button from './Button';

describe('Button', () => {
  it('renders primary variant', () => {
    render(<Button variant="primary">Click me</Button>);
    expect(screen.getByRole('button', { name: 'Click me' })).toBeInTheDocument();
  });

  it('renders secondary variant', () => {
    render(<Button variant="secondary">Click me</Button>);
    const button = screen.getByRole('button', { name: 'Click me' });
    expect(button.className).toContain('variantSecondary');
  });

  it('renders ghost variant', () => {
    render(<Button variant="ghost">Click me</Button>);
    const button = screen.getByRole('button', { name: 'Click me' });
    expect(button.className).toContain('variantGhost');
  });

  it('renders danger variant', () => {
    render(<Button variant="danger">Click me</Button>);
    const button = screen.getByRole('button', { name: 'Click me' });
    expect(button.className).toContain('variantDanger');
  });

  it('renders with size classes', () => {
    const { rerender } = render(<Button size="sm">Small</Button>);
    expect(screen.getByRole('button').className).toContain('sizeSm');

    rerender(<Button size="lg">Large</Button>);
    expect(screen.getByRole('button').className).toContain('sizeLg');
  });

  it('shows loading state', () => {
    render(<Button isLoading>Click me</Button>);
    const button = screen.getByRole('button');
    expect(button).toBeDisabled();
    expect(button.querySelector('.spinner')).toBeInTheDocument();
  });

  it('handles click events', () => {
    const handleClick = vi.fn();
    render(<Button onClick={handleClick}>Click me</Button>);
    fireEvent.click(screen.getByRole('button'));
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('is disabled when disabled prop is set', () => {
    render(<Button isDisabled>Click me</Button>);
    expect(screen.getByRole('button')).toBeDisabled();
  });

  it('renders left and right icons', () => {
    render(
      <Button leftIcon={<span data-testid="left">L</span>} rightIcon={<span data-testid="right">R</span>}>
        Icon Button
      </Button>
    );
    expect(screen.getByTestId('left')).toBeInTheDocument();
    expect(screen.getByTestId('right')).toBeInTheDocument();
  });

  it('hides icons when loading', () => {
    render(
      <Button isLoading leftIcon={<span data-testid="left">L</span>} rightIcon={<span data-testid="right">R</span>}>
        Loading
      </Button>
    );
    expect(screen.queryByTestId('left')).not.toBeInTheDocument();
    expect(screen.queryByTestId('right')).not.toBeInTheDocument();
  });
});

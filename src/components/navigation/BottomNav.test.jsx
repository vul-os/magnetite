/**
 * BottomNav.test.jsx — Tests for the mobile BottomNav component.
 *
 * Imports the REAL component (src/components/BottomNav.jsx) and renders it inside
 * a MemoryRouter to exercise routing-aware active state.
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import BottomNav from '../BottomNav';

function renderAt(path) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <BottomNav />
    </MemoryRouter>,
  );
}

describe('BottomNav', () => {
  it('renders a labelled navigation landmark', () => {
    renderAt('/home');
    expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument();
  });

  it('shows all five destinations', () => {
    renderAt('/home');
    for (const label of ['Home', 'Store', 'Play', 'Communities', 'Profile']) {
      expect(screen.getByRole('link', { name: label })).toBeInTheDocument();
    }
  });

  it('marks the active destination with aria-current=page', () => {
    renderAt('/marketplace');
    const store = screen.getByRole('link', { name: 'Store' });
    expect(store).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('link', { name: 'Home' })).not.toHaveAttribute('aria-current', 'page');
  });

  it('highlights Home on the root path', () => {
    renderAt('/');
    expect(screen.getByRole('link', { name: 'Home' })).toHaveAttribute('aria-current', 'page');
  });

  it('highlights Play on a /play/:id route', () => {
    renderAt('/play/abc123');
    expect(screen.getByRole('link', { name: 'Play' })).toHaveAttribute('aria-current', 'page');
  });

  it('highlights Profile on a /profile/:username route', () => {
    renderAt('/profile/alice');
    expect(screen.getByRole('link', { name: 'Profile' })).toHaveAttribute('aria-current', 'page');
  });

  it('every tab links to its destination path', () => {
    renderAt('/home');
    expect(screen.getByRole('link', { name: 'Home' })).toHaveAttribute('href', '/home');
    expect(screen.getByRole('link', { name: 'Store' })).toHaveAttribute('href', '/marketplace');
    expect(screen.getByRole('link', { name: 'Play' })).toHaveAttribute('href', '/play');
    expect(screen.getByRole('link', { name: 'Communities' })).toHaveAttribute('href', '/communities');
    expect(screen.getByRole('link', { name: 'Profile' })).toHaveAttribute('href', '/profile');
  });
});

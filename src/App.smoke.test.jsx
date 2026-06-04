import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import App from './App';

// Regression guard for the class of bug where a component using router hooks
// (e.g. BottomNav's useLocation) is mounted OUTSIDE <BrowserRouter> — which
// throws "useLocation() may be used only in the context of a <Router>" the
// moment the app renders. Mounting the whole <App/> here would have failed on
// that error, so this test pins BottomNav (and every always-mounted component)
// inside the router.
describe('App smoke', () => {
  it('mounts the whole app without throwing (router hooks are inside <Router>)', () => {
    const { container, unmount } = render(<App />);
    // If render() threw (e.g. useLocation outside Router) we never get here.
    expect(container).toBeTruthy();
    // BottomNav renders a bottom navigation <nav>; its presence proves a
    // router-hook component mounted inside BrowserRouter.
    expect(document.querySelector('nav')).toBeTruthy();
    unmount();
  });
});

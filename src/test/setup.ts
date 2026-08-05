import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';
import '@testing-library/jest-dom';

// jsdom lacks these observers; provide constructor-shaped stubs so components
// (e.g. GameDetail's sticky buy-bar IntersectionObserver) can `new` them.
class MockObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}
if (typeof globalThis.IntersectionObserver === 'undefined') {
  // MockObserver is intentionally a minimal stub, not a full IntersectionObserver
  // (it lacks root/rootMargin/thresholds) — cast through `unknown` rather than
  // pad it with unused dummy properties just to satisfy the DOM lib type.
  globalThis.IntersectionObserver = MockObserver as unknown as typeof IntersectionObserver;
}
if (typeof globalThis.ResizeObserver === 'undefined') {
  // Unlike IntersectionObserver above, ResizeObserver's constructor shape is
  // exactly { observe, unobserve, disconnect } — MockObserver already
  // satisfies it structurally, no cast needed.
  globalThis.ResizeObserver = MockObserver;
}

// jsdom doesn't implement matchMedia; provide a no-op MediaQueryList so
// useMediaQuery (and anything else) can mount. Defaults to "does not match".
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string): MediaQueryList => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent() {
      return false;
    },
  });
}

afterEach(() => {
  cleanup();
});

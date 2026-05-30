import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useGamepad } from './useGamepad';

// jsdom does not implement requestAnimationFrame natively in all test environments.
// Provide a stable fake that runs the callback once synchronously.
const rafCallbacks = [];
let rafIdCounter = 0;

beforeEach(() => {
  rafCallbacks.length = 0;
  rafIdCounter = 0;

  vi.stubGlobal('requestAnimationFrame', (cb) => {
    const id = ++rafIdCounter;
    rafCallbacks.push({ id, cb });
    return id;
  });

  vi.stubGlobal('cancelAnimationFrame', (id) => {
    const idx = rafCallbacks.findIndex((r) => r.id === id);
    if (idx !== -1) rafCallbacks.splice(idx, 1);
  });

  // Default: no gamepads connected.
  vi.stubGlobal('navigator', {
    ...navigator,
    getGamepads: vi.fn().mockReturnValue([]),
  });

  localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('useGamepad', () => {
  it('returns the expected API surface', () => {
    const { result } = renderHook(() => useGamepad());
    const api = result.current;

    expect(typeof api.gamepads).toBe('object');
    expect(typeof api.axes).toBe('object');
    expect(typeof api.buttons).toBe('object');
    expect(typeof api.bindings).toBe('object');
    expect(typeof api.listening).toBe('object'); // null initially
    expect(typeof api.setActiveGamepad).toBe('function');
    expect(typeof api.startListening).toBe('function');
    expect(typeof api.cancelListening).toBe('function');
    expect(typeof api.clearBinding).toBe('function');
    expect(typeof api.resetBindings).toBe('function');
    expect(typeof api.isSupported).toBe('boolean');
  });

  it('isSupported is true when navigator.getGamepads is available', () => {
    const { result } = renderHook(() => useGamepad());
    expect(result.current.isSupported).toBe(true);
  });

  it('initial state: gamepads is an empty array, no listening action', () => {
    const { result } = renderHook(() => useGamepad());
    expect(Array.isArray(result.current.gamepads)).toBe(true);
    expect(result.current.gamepads).toHaveLength(0);
    expect(result.current.listening).toBeNull();
  });

  it('startListening sets the listening action', () => {
    const { result } = renderHook(() => useGamepad());

    act(() => {
      result.current.startListening('fire');
    });

    expect(result.current.listening).toBe('fire');
  });

  it('cancelListening clears the listening action', () => {
    const { result } = renderHook(() => useGamepad());

    act(() => {
      result.current.startListening('jump');
    });

    expect(result.current.listening).toBe('jump');

    act(() => {
      result.current.cancelListening();
    });

    expect(result.current.listening).toBeNull();
  });

  it('default bindings include fire and jump', () => {
    const { result } = renderHook(() => useGamepad());
    const { bindings } = result.current;

    expect(bindings).toHaveProperty('fire');
    expect(bindings['fire'].type).toBe('button');
    expect(bindings['fire'].index).toBe(7); // R2

    expect(bindings).toHaveProperty('jump');
    expect(bindings['jump'].type).toBe('button');
    expect(bindings['jump'].index).toBe(0); // Cross/A
  });

  it('default bindings include axis-based actions', () => {
    const { result } = renderHook(() => useGamepad());
    const { bindings } = result.current;

    expect(bindings['move_forward'].type).toBe('axis');
    expect(bindings['aim_horizontal'].type).toBe('axis');
  });

  it('clearBinding removes a binding', () => {
    const { result } = renderHook(() => useGamepad());

    expect(result.current.bindings['fire']).toBeDefined();

    act(() => {
      result.current.clearBinding('fire');
    });

    expect(result.current.bindings['fire']).toBeUndefined();
  });

  it('resetBindings restores defaults', () => {
    const { result } = renderHook(() => useGamepad());

    // Clear a couple of bindings first
    act(() => {
      result.current.clearBinding('fire');
      result.current.clearBinding('jump');
    });

    expect(result.current.bindings['fire']).toBeUndefined();

    act(() => {
      result.current.resetBindings();
    });

    expect(result.current.bindings['fire']).toBeDefined();
    expect(result.current.bindings['jump']).toBeDefined();
  });

  it('persists bindings to localStorage via resetBindings', () => {
    const { result } = renderHook(() => useGamepad());

    act(() => {
      result.current.resetBindings();
    });

    const stored = localStorage.getItem('magnetite_gamepad_bindings');
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored);
    expect(parsed).toHaveProperty('fire');
  });

  it('persists bindings to localStorage via clearBinding', () => {
    const { result } = renderHook(() => useGamepad());

    act(() => {
      result.current.clearBinding('reload');
    });

    const stored = localStorage.getItem('magnetite_gamepad_bindings');
    expect(stored).not.toBeNull();
    const parsed = JSON.parse(stored);
    expect(parsed['reload']).toBeUndefined();
  });

  it('loads bindings from localStorage on mount', () => {
    const customBindings = {
      fire: { type: 'button', index: 5 },
      jump: { type: 'button', index: 1 },
    };
    localStorage.setItem('magnetite_gamepad_bindings', JSON.stringify(customBindings));

    const { result } = renderHook(() => useGamepad());
    expect(result.current.bindings['fire'].index).toBe(5);
    expect(result.current.bindings['jump'].index).toBe(1);
  });

  it('handles corrupt localStorage gracefully (uses defaults)', () => {
    localStorage.setItem('magnetite_gamepad_bindings', 'NOT_JSON');

    const { result } = renderHook(() => useGamepad());
    // Should not throw and should have default bindings
    expect(result.current.bindings).toHaveProperty('fire');
  });

  it('setActiveGamepad updates the active index', () => {
    const { result } = renderHook(() => useGamepad());
    expect(result.current.activeGamepad).toBe(0);

    act(() => {
      result.current.setActiveGamepad(1);
    });

    expect(result.current.activeGamepad).toBe(1);
  });

  it('dispatches gamepadconnected and gamepaddisconnected events without crashing', () => {
    const { result, unmount } = renderHook(() => useGamepad());
    expect(result.current.gamepads).toHaveLength(0);

    act(() => {
      window.dispatchEvent(new Event('gamepadconnected'));
      window.dispatchEvent(new Event('gamepaddisconnected'));
    });

    // No error thrown; hook still intact
    expect(result.current.bindings).toBeDefined();
    unmount();
  });
});

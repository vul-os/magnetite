import { useState, useEffect, useCallback, useRef } from 'react';

/**
 * useGamepad — Web Gamepad API hook.
 *
 * Polls connected gamepads at ~60 fps, exposes:
 *  - gamepads:       Array of connected Gamepad objects (snapshot each frame)
 *  - activeGamepad:  Index of the selected gamepad
 *  - axes:           Float32Array of axis values for activeGamepad
 *  - buttons:        Array<{ pressed, touched, value }> for activeGamepad
 *  - bindings:       Map of action → { gamepadIndex, buttonIndex | axisIndex, type }
 *  - setActiveGamepad, updateBinding, clearBinding, resetBindings
 */

const DEFAULT_BINDINGS = {
  move_forward:   { type: 'axis',   index: 1, invert: true  },
  move_backward:  { type: 'axis',   index: 1, invert: false },
  move_left:      { type: 'axis',   index: 0, invert: true  },
  move_right:     { type: 'axis',   index: 0, invert: false },
  aim_horizontal: { type: 'axis',   index: 2, invert: false },
  aim_vertical:   { type: 'axis',   index: 3, invert: false },
  fire:           { type: 'button', index: 7  },   // R2
  aim:            { type: 'button', index: 6  },   // L2
  jump:           { type: 'button', index: 0  },   // Cross / A
  interact:       { type: 'button', index: 2  },   // Square / X
  reload:         { type: 'button', index: 3  },   // Triangle / Y
  sprint:         { type: 'button', index: 10 },   // L3
  map:            { type: 'button', index: 8  },   // Share / Select
  pause:          { type: 'button', index: 9  },   // Options / Start
};

const STORAGE_KEY = 'magnetite_gamepad_bindings';

function loadBindings() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : DEFAULT_BINDINGS;
  } catch {
    return DEFAULT_BINDINGS;
  }
}

function saveBindings(bindings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(bindings));
  } catch { /* ignore */ }
}

export function useGamepad() {
  const [gamepads, setGamepads]         = useState([]);
  const [activeGamepad, setActiveGamepad] = useState(0);
  const [axes, setAxes]                 = useState([]);
  const [buttons, setButtons]           = useState([]);
  const [bindings, setBindings]         = useState(loadBindings);
  const [listening, setListening]       = useState(null); // action key being re-bound
  const rafRef                          = useRef(null);
  const mountedRef                      = useRef(true);

  // Poll gamepads
  useEffect(() => {
    mountedRef.current = true;

    function poll() {
      if (!mountedRef.current) return;
      const raw = navigator.getGamepads ? Array.from(navigator.getGamepads()).filter(Boolean) : [];
      setGamepads(raw.map(g => ({
        id: g.id,
        index: g.index,
        connected: g.connected,
        buttonCount: g.buttons.length,
        axisCount: g.axes.length,
      })));

      const gp = raw[activeGamepad];
      if (gp) {
        setAxes(Array.from(gp.axes));
        setButtons(gp.buttons.map(b => ({ pressed: b.pressed, touched: b.touched, value: b.value })));
      } else {
        setAxes([]);
        setButtons([]);
      }
      rafRef.current = requestAnimationFrame(poll);
    }

    rafRef.current = requestAnimationFrame(poll);

    const onConnect    = () => { /* poll picks it up */ };
    const onDisconnect = () => { /* poll picks it up */ };
    window.addEventListener('gamepadconnected', onConnect);
    window.addEventListener('gamepaddisconnected', onDisconnect);

    return () => {
      mountedRef.current = false;
      cancelAnimationFrame(rafRef.current);
      window.removeEventListener('gamepadconnected', onConnect);
      window.removeEventListener('gamepaddisconnected', onDisconnect);
    };
  }, [activeGamepad]);

  // While listening, detect first pressed button/axis
  useEffect(() => {
    if (!listening) return;

    let rafId;
    function detectInput() {
      const raw = navigator.getGamepads ? Array.from(navigator.getGamepads()).filter(Boolean) : [];
      const gp  = raw[activeGamepad];
      if (gp) {
        // Check buttons
        for (let i = 0; i < gp.buttons.length; i++) {
          if (gp.buttons[i].pressed) {
            setBindings(prev => {
              const next = { ...prev, [listening]: { type: 'button', index: i } };
              saveBindings(next);
              return next;
            });
            setListening(null);
            return;
          }
        }
        // Check axes with threshold
        for (let i = 0; i < gp.axes.length; i++) {
          if (Math.abs(gp.axes[i]) > 0.7) {
            setBindings(prev => {
              const next = { ...prev, [listening]: { type: 'axis', index: i, invert: gp.axes[i] < 0 } };
              saveBindings(next);
              return next;
            });
            setListening(null);
            return;
          }
        }
      }
      rafId = requestAnimationFrame(detectInput);
    }
    rafId = requestAnimationFrame(detectInput);
    return () => cancelAnimationFrame(rafId);
  }, [listening, activeGamepad]);

  const startListening = useCallback((action) => setListening(action), []);
  const cancelListening = useCallback(() => setListening(null), []);

  const clearBinding = useCallback((action) => {
    setBindings(prev => {
      const next = { ...prev };
      delete next[action];
      saveBindings(next);
      return next;
    });
  }, []);

  const resetBindings = useCallback(() => {
    setBindings(DEFAULT_BINDINGS);
    saveBindings(DEFAULT_BINDINGS);
  }, []);

  return {
    gamepads,
    activeGamepad,
    setActiveGamepad,
    axes,
    buttons,
    bindings,
    listening,
    startListening,
    cancelListening,
    clearBinding,
    resetBindings,
    isSupported: typeof navigator !== 'undefined' && 'getGamepads' in navigator,
  };
}

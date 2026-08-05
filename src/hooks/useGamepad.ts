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

export type BindingAction =
  | 'move_forward' | 'move_backward' | 'move_left' | 'move_right'
  | 'aim_horizontal' | 'aim_vertical'
  | 'fire' | 'aim' | 'jump' | 'interact' | 'reload' | 'sprint' | 'map' | 'pause';

export interface AxisBinding {
  type: 'axis';
  index: number;
  invert: boolean;
}

export interface ButtonBinding {
  type: 'button';
  index: number;
}

export type Binding = AxisBinding | ButtonBinding;

export type Bindings = Partial<Record<BindingAction, Binding>>;

const DEFAULT_BINDINGS: Bindings = {
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

function loadBindings(): Bindings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Bindings) : DEFAULT_BINDINGS;
  } catch {
    return DEFAULT_BINDINGS;
  }
}

function saveBindings(bindings: Bindings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(bindings));
  } catch { /* ignore */ }
}

export interface GamepadSnapshot {
  id: string;
  index: number;
  connected: boolean;
  buttonCount: number;
  axisCount: number;
}

export interface ButtonState {
  pressed: boolean;
  touched: boolean;
  value: number;
}

export function useGamepad() {
  const [gamepads, setGamepads]         = useState<GamepadSnapshot[]>([]);
  const [activeGamepad, setActiveGamepad] = useState(0);
  const [axes, setAxes]                 = useState<number[]>([]);
  const [buttons, setButtons]           = useState<ButtonState[]>([]);
  const [bindings, setBindings]         = useState<Bindings>(loadBindings);
  const [listening, setListening]       = useState<BindingAction | null>(null); // action key being re-bound
  const rafRef                          = useRef<number | null>(null);
  const mountedRef                      = useRef(true);

  // Poll gamepads
  useEffect(() => {
    mountedRef.current = true;

    function poll() {
      if (!mountedRef.current) return;
      const raw: Gamepad[] = navigator.getGamepads ? Array.from(navigator.getGamepads()).filter((g): g is Gamepad => g !== null) : [];
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
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      window.removeEventListener('gamepadconnected', onConnect);
      window.removeEventListener('gamepaddisconnected', onDisconnect);
    };
  }, [activeGamepad]);

  // While listening, detect first pressed button/axis
  useEffect(() => {
    if (!listening) return;

    let rafId: number;
    function detectInput() {
      const raw: Gamepad[] = navigator.getGamepads ? Array.from(navigator.getGamepads()).filter((g): g is Gamepad => g !== null) : [];
      const gp  = raw[activeGamepad];
      if (gp && listening) {
        // Check buttons
        for (let i = 0; i < gp.buttons.length; i++) {
          if (gp.buttons[i].pressed) {
            setBindings(prev => {
              const next: Bindings = { ...prev, [listening]: { type: 'button', index: i } };
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
              const next: Bindings = { ...prev, [listening]: { type: 'axis', index: i, invert: gp.axes[i] < 0 } };
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

  const startListening = useCallback((action: BindingAction) => setListening(action), []);
  const cancelListening = useCallback(() => setListening(null), []);

  const clearBinding = useCallback((action: BindingAction) => {
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

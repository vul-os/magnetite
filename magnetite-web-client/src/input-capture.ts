/**
 * magnetite-web-client/src/input-capture.ts
 *
 * Captures keyboard and mouse events and maintains a current InputFrame
 * that can be polled each tick and sent to the server.
 *
 * Mirrors the KeyState + MouseState snapshot model in magnetite-sdk::input.
 */

import { defaultKeyState, defaultMouseState } from './protocol';
import type { KeyState, MouseState, Input } from './types';

// ---------------------------------------------------------------------------
// InputCapture
// ---------------------------------------------------------------------------

export class InputCapture {
  private _target: EventTarget | null;
  private _keys: KeyState;
  private _mouse: MouseState;
  private _attached: boolean;

  private _onKeyDown: (e: KeyboardEvent) => void;
  private _onKeyUp: (e: KeyboardEvent) => void;
  private _onMouseMove: (e: MouseEvent) => void;
  private _onMouseDown: (e: MouseEvent) => void;
  private _onMouseUp: (e: MouseEvent) => void;
  private _onWheel: (e: WheelEvent) => void;
  private _onContextMenu: (e: Event) => void;

  /**
   * @param target - DOM element to attach listeners to (defaults to `window`).
   *   For a canvas game, pass the canvas element.
   */
  constructor(target?: EventTarget | null) {
    this._target = target || (typeof window !== 'undefined' ? window : null);
    this._keys = defaultKeyState();
    this._mouse = defaultMouseState();
    this._attached = false;

    // Bound handler refs for cleanup
    this._onKeyDown = this._handleKeyDown.bind(this);
    this._onKeyUp = this._handleKeyUp.bind(this);
    this._onMouseMove = this._handleMouseMove.bind(this);
    this._onMouseDown = this._handleMouseDown.bind(this);
    this._onMouseUp = this._handleMouseUp.bind(this);
    this._onWheel = this._handleWheel.bind(this);
    this._onContextMenu = (e: Event) => e.preventDefault();
  }

  // --------------------------------------------------------------------------
  // Lifecycle
  // --------------------------------------------------------------------------

  /** Attach event listeners. Call once after mount. */
  attach(): void {
    if (this._attached || !this._target) return;
    this._target.addEventListener('keydown', this._onKeyDown as EventListener);
    this._target.addEventListener('keyup', this._onKeyUp as EventListener);
    this._target.addEventListener('mousemove', this._onMouseMove as EventListener);
    this._target.addEventListener('mousedown', this._onMouseDown as EventListener);
    this._target.addEventListener('mouseup', this._onMouseUp as EventListener);
    this._target.addEventListener('wheel', this._onWheel as EventListener, { passive: true });
    this._target.addEventListener('contextmenu', this._onContextMenu);
    this._attached = true;
  }

  /** Remove event listeners. Call on cleanup / unmount. */
  detach(): void {
    if (!this._attached || !this._target) return;
    this._target.removeEventListener('keydown', this._onKeyDown as EventListener);
    this._target.removeEventListener('keyup', this._onKeyUp as EventListener);
    this._target.removeEventListener('mousemove', this._onMouseMove as EventListener);
    this._target.removeEventListener('mousedown', this._onMouseDown as EventListener);
    this._target.removeEventListener('mouseup', this._onMouseUp as EventListener);
    this._target.removeEventListener('wheel', this._onWheel as EventListener);
    this._target.removeEventListener('contextmenu', this._onContextMenu);
    this._attached = false;
  }

  // --------------------------------------------------------------------------
  // Snapshot
  // --------------------------------------------------------------------------

  /**
   * Return a snapshot of the current input state and RESET delta values
   * (mouse delta and scroll) so they accumulate only within one tick.
   */
  snapshot(seq: number, timestampMs: number): Input {
    const input: Input = {
      keys: { ...this._keys },
      mouse: { ...this._mouse },
      sequence: seq,
      timestamp_ms: timestampMs,
    };

    // Reset per-frame delta values
    this._mouse.delta_x = 0;
    this._mouse.delta_y = 0;
    this._mouse.scroll = 0;

    return input;
  }

  // --------------------------------------------------------------------------
  // Event handlers
  // --------------------------------------------------------------------------

  private _handleKeyDown(e: KeyboardEvent): void {
    const k = _keyToField(e.code);
    if (k && k in this._keys) {
      this._keys[k] = true;
      e.preventDefault();
    }
  }

  private _handleKeyUp(e: KeyboardEvent): void {
    const k = _keyToField(e.code);
    if (k && k in this._keys) {
      this._keys[k] = false;
    }
  }

  private _handleMouseMove(e: MouseEvent): void {
    this._mouse.x = e.clientX;
    this._mouse.y = e.clientY;
    this._mouse.delta_x += e.movementX || 0;
    this._mouse.delta_y += e.movementY || 0;
  }

  private _handleMouseDown(e: MouseEvent): void {
    if (e.button === 0) this._mouse.left_button = true;
    if (e.button === 1) this._mouse.middle_button = true;
    if (e.button === 2) this._mouse.right_button = true;
  }

  private _handleMouseUp(e: MouseEvent): void {
    if (e.button === 0) this._mouse.left_button = false;
    if (e.button === 1) this._mouse.middle_button = false;
    if (e.button === 2) this._mouse.right_button = false;
  }

  private _handleWheel(e: WheelEvent): void {
    this._mouse.scroll += e.deltaY;
  }
}

// ---------------------------------------------------------------------------
// KeyCode → KeyState field mapping
// ---------------------------------------------------------------------------

/**
 * Map a browser KeyboardEvent.code to a KeyState field name.
 *
 * Matches the KeyCode enum in magnetite_sdk::input:
 *   Forward  = W / ArrowUp
 *   Backward = S / ArrowDown
 *   Left     = A / ArrowLeft
 *   Right    = D / ArrowRight
 *   Jump     = Space
 *   Crouch   = ControlLeft / ControlRight / KeyC
 *   Attack   = (left mouse — handled as mouse button, but also KeyZ)
 *   SecondaryAttack = (right mouse or KeyX)
 *   Interact = KeyR / KeyE
 *   Sprint   = ShiftLeft / ShiftRight
 *
 * @param code - KeyboardEvent.code
 * @returns KeyState field name or null
 */
function _keyToField(code: string): keyof KeyState | null {
  switch (code) {
    case 'KeyW':
    case 'ArrowUp':
      return 'forward';
    case 'KeyS':
    case 'ArrowDown':
      return 'backward';
    case 'KeyA':
    case 'ArrowLeft':
      return 'left';
    case 'KeyD':
    case 'ArrowRight':
      return 'right';
    case 'Space':
      return 'jump';
    case 'ControlLeft':
    case 'ControlRight':
    case 'KeyC':
      return 'crouch';
    case 'KeyZ':
      return 'attack';
    case 'KeyX':
      return 'secondary_attack';
    case 'KeyR':
    case 'KeyE':
      return 'interact';
    case 'ShiftLeft':
    case 'ShiftRight':
      return 'sprint';
    default:
      return null;
  }
}

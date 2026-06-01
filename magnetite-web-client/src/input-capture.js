/**
 * magnetite-web-client/src/input-capture.js
 *
 * Captures keyboard and mouse events and maintains a current InputFrame
 * that can be polled each tick and sent to the server.
 *
 * Mirrors the KeyState + MouseState snapshot model in magnetite-sdk::input.
 */

import { defaultKeyState, defaultMouseState } from './protocol.js';

// ---------------------------------------------------------------------------
// InputCapture
// ---------------------------------------------------------------------------

export class InputCapture {
  /**
   * @param {EventTarget} [target=window] - DOM element to attach listeners to.
   *   For a canvas game, pass the canvas element.
   */
  constructor(target) {
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
    this._onContextMenu = (e) => e.preventDefault();
  }

  // --------------------------------------------------------------------------
  // Lifecycle
  // --------------------------------------------------------------------------

  /** Attach event listeners. Call once after mount. */
  attach() {
    if (this._attached || !this._target) return;
    this._target.addEventListener('keydown', this._onKeyDown);
    this._target.addEventListener('keyup', this._onKeyUp);
    this._target.addEventListener('mousemove', this._onMouseMove);
    this._target.addEventListener('mousedown', this._onMouseDown);
    this._target.addEventListener('mouseup', this._onMouseUp);
    this._target.addEventListener('wheel', this._onWheel, { passive: true });
    this._target.addEventListener('contextmenu', this._onContextMenu);
    this._attached = true;
  }

  /** Remove event listeners. Call on cleanup / unmount. */
  detach() {
    if (!this._attached || !this._target) return;
    this._target.removeEventListener('keydown', this._onKeyDown);
    this._target.removeEventListener('keyup', this._onKeyUp);
    this._target.removeEventListener('mousemove', this._onMouseMove);
    this._target.removeEventListener('mousedown', this._onMouseDown);
    this._target.removeEventListener('mouseup', this._onMouseUp);
    this._target.removeEventListener('wheel', this._onWheel);
    this._target.removeEventListener('contextmenu', this._onContextMenu);
    this._attached = false;
  }

  // --------------------------------------------------------------------------
  // Snapshot
  // --------------------------------------------------------------------------

  /**
   * Return a snapshot of the current input state and RESET delta values
   * (mouse delta and scroll) so they accumulate only within one tick.
   *
   * @param {number} seq
   * @param {number} timestampMs
   * @returns {import('./types.js').Input}
   */
  snapshot(seq, timestampMs) {
    const input = {
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

  _handleKeyDown(e) {
    const k = _keyToField(e.code);
    if (k && k in this._keys) {
      this._keys[k] = true;
      e.preventDefault();
    }
  }

  _handleKeyUp(e) {
    const k = _keyToField(e.code);
    if (k && k in this._keys) {
      this._keys[k] = false;
    }
  }

  _handleMouseMove(e) {
    this._mouse.x = e.clientX;
    this._mouse.y = e.clientY;
    this._mouse.delta_x += e.movementX || 0;
    this._mouse.delta_y += e.movementY || 0;
  }

  _handleMouseDown(e) {
    if (e.button === 0) this._mouse.left_button = true;
    if (e.button === 1) this._mouse.middle_button = true;
    if (e.button === 2) this._mouse.right_button = true;
  }

  _handleMouseUp(e) {
    if (e.button === 0) this._mouse.left_button = false;
    if (e.button === 1) this._mouse.middle_button = false;
    if (e.button === 2) this._mouse.right_button = false;
  }

  _handleWheel(e) {
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
 * @param {string} code - KeyboardEvent.code
 * @returns {string | null} KeyState field name or null
 */
function _keyToField(code) {
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

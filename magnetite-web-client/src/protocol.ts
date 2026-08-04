/**
 * magnetite-web-client/src/protocol.ts
 *
 * Wire-format helpers that exactly mirror magnetite-sdk::protocol.
 *
 * ServerNet (server → client), tagged with "type" (snake_case):
 *   welcome        { player_id, config }
 *   snapshot       { tick, full }          // full = base64-encoded JSON bytes
 *   delta          { tick, since_tick, diff } // diff = base64-encoded JSON bytes
 *   ack            { seq, tick }
 *   reject         { seq, reason }
 *
 * ClientNet (client → server), tagged with "type" (snake_case):
 *   input_frame    { seq, tick, input }
 *
 * Input frame shape (mirrors magnetite_sdk::input::Input):
 *   { keys: KeyState, mouse: MouseState, sequence: u64, timestamp_ms: u64 }
 *
 * KeyState: { forward, backward, left, right, jump, crouch, attack,
 *              secondary_attack, interact, sprint }  (all bool)
 *
 * MouseState: { x, y, delta_x, delta_y, left_button, right_button,
 *               middle_button, scroll }
 */

import type { KeyState, MouseState, Input, ServerMessage } from './types';

// ---------------------------------------------------------------------------
// Default constructors
// ---------------------------------------------------------------------------

export function defaultKeyState(): KeyState {
  return {
    forward: false,
    backward: false,
    left: false,
    right: false,
    jump: false,
    crouch: false,
    attack: false,
    secondary_attack: false,
    interact: false,
    sprint: false,
  };
}

export function defaultMouseState(): MouseState {
  return {
    x: 0,
    y: 0,
    delta_x: 0,
    delta_y: 0,
    left_button: false,
    right_button: false,
    middle_button: false,
    scroll: 0,
  };
}

export function defaultInput(): Input {
  return {
    keys: defaultKeyState(),
    mouse: defaultMouseState(),
    sequence: 0,
    timestamp_ms: 0,
  };
}

// ---------------------------------------------------------------------------
// Build a ClientNet::InputFrame message
// ---------------------------------------------------------------------------

/**
 * @param seq  - client-local sequence number (u32)
 * @param tick - authoritative tick this input targets (u64)
 * @returns JSON string ready to send over WebSocket
 */
export function encodeInputFrame(seq: number, tick: number, input: Input): string {
  return JSON.stringify({
    type: 'input_frame',
    seq,
    tick,
    input,
  });
}

// ---------------------------------------------------------------------------
// Parse a ServerNet message
// ---------------------------------------------------------------------------

/**
 * Parse a raw WebSocket text message into a typed ServerNet variant.
 *
 * Returns null (and logs a warning) if the message cannot be parsed.
 */
export function parseServerMessage(raw: string): ServerMessage | null {
  try {
    const msg = JSON.parse(raw);
    if (typeof msg !== 'object' || msg === null || typeof msg.type !== 'string') {
      console.warn('[magnetite] unexpected server message shape:', raw);
      return null;
    }
    return msg;
  } catch (e) {
    console.warn('[magnetite] failed to parse server message:', e, raw);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Decode opaque bytes fields (base64 → JSON object)
// ---------------------------------------------------------------------------

/**
 * The server sends `full` (Snapshot) and `diff` (Delta) as Vec<u8> which
 * serde_json serialises as a base64 string inside the JSON envelope.
 *
 * Decodes that base64 string back to a JS object.
 *
 * @param bytesField  - base64 string or array of bytes
 */
export function decodeBytes(bytesField: string | number[] | null | undefined): unknown | null {
  if (!bytesField) return null;
  try {
    if (Array.isArray(bytesField)) {
      // serde_json may also emit u8 arrays directly
      const str = new TextDecoder().decode(new Uint8Array(bytesField));
      return JSON.parse(str);
    }
    if (typeof bytesField === 'string') {
      const binary = atob(bytesField);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      const str = new TextDecoder().decode(bytes);
      return JSON.parse(str);
    }
    return null;
  } catch (e) {
    console.warn('[magnetite] failed to decode bytes field:', e);
    return null;
  }
}

# magnetite-web-client

A lightweight, dependency-free ES module browser client that speaks the
Magnetite authoritative protocol, enabling Magnetite games to be played in a
browser tab.

## Quick start

```html
<script type="module">
  import { createClient } from './magnetite-web-client/src/client.js';

  const client = createClient({
    url:    'ws://localhost:9001',   // from `magnetite dev`
    token:  'optional-jwt',
    canvas: document.getElementById('game'),
  });

  client.connect();
  client.onState(state => console.log('predicted state:', state));
</script>
```

Open `demo/index.html` in a browser and point it at a `magnetite dev` server.

## Public API

### `createClient(opts) → MagnetiteClient`

| Option        | Type                       | Default      | Description                                |
|---------------|----------------------------|--------------|--------------------------------------------|
| `url`         | `string`                   | required     | WebSocket URL (`ws://` or `wss://`)        |
| `token`       | `string`                   | —            | Optional auth token (appended as `?token=`) |
| `canvas`      | `HTMLCanvasElement`        | —            | Canvas to render into                      |
| `render`      | `(ctx, state, pid) => void` | arena renderer | Custom render function                  |
| `applyInput`  | `(state, input, tick) => state` | arena predictor | Custom prediction function           |
| `autoReconnect` | `boolean`               | `true`       | Reconnect with exponential backoff         |

### `MagnetiteClient`

| Method / Property | Description |
|---|---|
| `connect()` | Open the WebSocket connection |
| `disconnect()` | Close and stop the tick loop |
| `sendInput(input)` | Send a pre-built `Input` frame (advanced) |
| `onState(fn)` | Subscribe to state updates; returns unsubscribe function |
| `playerId` | Local player id string (set after `Welcome`) |
| `matchConfig` | `MatchConfig` object from the server |
| `state` | Current predicted state |

## Protocol mapping

The client exactly mirrors the frozen `magnetite-sdk::protocol` wire shapes.

### Server → Client (`ServerNet`, tagged `"type"` snake_case)

| Message     | Fields                        | Client action                              |
|-------------|-------------------------------|--------------------------------------------|
| `welcome`   | `player_id, config`           | Set player id; start tick loop             |
| `snapshot`  | `tick, full`                  | Full authoritative reset; `PredictionBuffer.applySnapshot` |
| `delta`     | `tick, since_tick, diff`      | Apply diff; update predicted state         |
| `ack`       | `seq, tick`                   | `PredictionBuffer.ack(seq, tick)` — discard confirmed frames |
| `reject`    | `seq, reason`                 | `PredictionBuffer.reject(seq)` — re-reconcile |

### Client → Server (`ClientNet`)

| Message       | Fields             | Wire shape                                  |
|---------------|--------------------|---------------------------------------------|
| `input_frame` | `seq, tick, input` | `{ "type": "input_frame", "seq": 1, "tick": 42, "input": {...} }` |

`input` is `magnetite_sdk::input::Input`:
```json
{
  "keys": {
    "forward": false, "backward": false, "left": false, "right": false,
    "jump": false, "crouch": false, "attack": false,
    "secondary_attack": false, "interact": false, "sprint": false
  },
  "mouse": {
    "x": 0, "y": 0, "delta_x": 0, "delta_y": 0,
    "left_button": false, "right_button": false,
    "middle_button": false, "scroll": 0
  },
  "sequence": 1,
  "timestamp_ms": 1717123456789
}
```

## Arena-shooter game types

The arena-shooter (`game-template-authoritative`) uses:

| Type             | Shape |
|------------------|-------|
| `ArenaSnapshot`  | `{ players: ShooterPlayer[], projectiles: Projectile[], tick }` |
| `ArenaDelta`     | `{ changed_players: ShooterPlayer[], removed_projectile_ids: u64[], new_projectiles: Projectile[] }` |
| `ArenaView`      | `{ self_state: ShooterPlayer|null, other_players: ShooterPlayer[], projectiles: Projectile[], tick }` |
| `ShooterPlayer`  | `{ id, x, y, angle, hp, alive, last_shot_tick, score }` |
| `Projectile`     | `{ id, owner, x, y, vx, vy, ticks_left }` |

The `full` (Snapshot) and `diff` (Delta) fields are serialized as `Vec<u8>` on
the server — `serde_json` encodes them as base64 strings. The client decodes
them automatically via `decodeBytes`.

## Prediction & reconciliation

Mirrors the `PredictionBuffer` in `magnetite-sdk::networking`:

1. On each tick the client applies the input locally via `applyInput`
   and buffers the `(seq, tick, input)` frame.
2. The frame is sent to the server as `InputFrame`.
3. On `Ack(seq, tick)`: frames with `seq ≤ acked` are discarded.
4. On `Snapshot(tick, full)`: the local state resets to the authoritative
   snapshot; all buffered frames with `tick > snapshot.tick` are replayed.
5. On `Reject(seq)`: the frame is discarded; state reconciles from authority.

## Input mapping (keyboard)

| Key            | `KeyState` field      |
|----------------|-----------------------|
| W / ArrowUp    | `forward`             |
| S / ArrowDown  | `backward`            |
| A / ArrowLeft  | `left`                |
| D / ArrowRight | `right`               |
| Space          | `jump`                |
| Ctrl / C       | `crouch`              |
| Z              | `attack`              |
| X              | `secondary_attack`    |
| R / E          | `interact`            |
| Shift          | `sprint`              |
| Left mouse     | `mouse.left_button`   |
| Right mouse    | `mouse.right_button`  |

## Pluggable renderer

Supply `render` to draw any game type:

```js
const client = createClient({
  url,
  canvas,
  render(ctx, state, playerId) {
    // state is your game's ArenaView / View shape
    ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    // ... draw your game
  },
  applyInput(state, input, tick) {
    // predict locally; return new state
    return { ...state, tick };
  },
});
```

## React integration

```jsx
import { createClient } from '../magnetite-web-client/src/client.js';
import { useEffect, useRef, useState } from 'react';

export function GameCanvas({ url }) {
  const canvasRef = useRef(null);
  const [gameState, setGameState] = useState(null);

  useEffect(() => {
    const client = createClient({ url, canvas: canvasRef.current });
    client.connect();
    const unsub = client.onState(setGameState);
    return () => { unsub(); client.disconnect(); };
  }, [url]);

  return <canvas ref={canvasRef} width={600} height={600} />;
}
```

## Import path

From the repo root:
```
magnetite-web-client/src/client.js
```

No build step required — all files are plain ES modules (`type="module"`).
For bundler projects (Vite/webpack) import the same path; the ES module
format is directly consumable.

## Running unit tests

Tests live alongside the source and run via the repo's Vitest setup:

```bash
# from repo root
npx vitest run magnetite-web-client/src/client.test.js
```

Or add to vitest.config.js `include` to run with `npm test`.

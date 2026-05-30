# Controllers & Gamepad Input

Magnetite provides first-class controller support through the SDK, covering both the
**Web Gamepad API** (browser WASM builds) and **gilrs** (native desktop builds).
A unified `InputMap` layer lets players rebind actions without changing game code.

---

## Quick start

```rust
use magnetite_sdk::input::gamepad::{GamepadButton, GamepadEvent, InputMap, GameAction};

// Default map: South (A) → Jump, North (Y) → Dash, etc.
let map = InputMap::default();

// Process an event from the platform (WASM or native).
let event = GamepadEvent::ButtonPressed(GamepadButton::South);
let actions = map.process_gamepad(&event);

assert!(actions.contains(&GameAction::Jump));
```

---

## `GameAction` — unified action set

| `GameAction` | Default gamepad binding | Notes |
|---|---|---|
| `MoveForward` | Left stick up | |
| `MoveBackward` | Left stick down | |
| `MoveLeft` | Left stick left | |
| `MoveRight` | Left stick right | |
| `Look { dx, dy }` | Right stick | Normalised -1..1 analogue look/aim |
| `Jump` | South (A / Cross) | |
| `Crouch` | Right stick click | |
| `Fire` | Right trigger (RT / R2) | Primary fire / interact |
| `AimDownSights` | Left trigger (LT / L2) | Secondary fire / ADS |
| `Reload` | West (X / Square) | |
| `Sprint` | Left bumper (LB / L1) | Modifier |
| `Pause` | Start | |
| `Accelerate` | Right trigger (RT / R2) | Motorsport / driving games |
| `Brake` | Left trigger (LT / L2) | Motorsport / driving games |
| `Steer(f32)` | Left stick X-axis | -1.0 = full left; 1.0 = full right |
| `Handbrake` | East (B / Circle) | Drift / handbrake |
| `Custom(String)` | — | Game-specific named action |

---

## `GamepadButton` enum

```rust
pub enum GamepadButton {
    South, North, East, West,
    LeftBumper, RightBumper,
    LeftTrigger, RightTrigger,    // digital press
    Select, Start,
    LeftStick, RightStick,        // stick click
    DPadUp, DPadDown, DPadLeft, DPadRight,
}
```

## `GamepadAxis` enum

```rust
pub enum GamepadAxis {
    LeftStickX, LeftStickY,
    RightStickX, RightStickY,
    LeftTrigger, RightTrigger,    // 0.0 … 1.0 analog
}
```

---

## Custom bindings

Create a custom `InputMap` to override defaults:

```rust
use magnetite_sdk::input::gamepad::{
    GameAction, GamepadButton, GamepadAxis, InputBinding, InputMap, InputSource,
};

let mut map = InputMap::default();

// Rebind Fire from right-trigger to right-bumper.
map.bind(
    GameAction::Fire,
    InputBinding::Gamepad(InputSource::Button(GamepadButton::RightBumper)),
);
```

Bindings can be saved as JSON for player preferences and loaded back at runtime.

---

## Web (WASM) — Gamepad API

In browser WASM builds the `useGamepad` React hook polls `navigator.getGamepads()` every
animation frame and emits `GamepadEvent`s to connected listeners.

```js
import { useGamepad } from '../hooks/useGamepad';

function MyComponent() {
  const { gamepads, onButton, onAxis } = useGamepad();

  useEffect(() => {
    const unsub = onButton('south', (pressed) => {
      if (pressed) sendAction('jump');
    });
    return unsub;
  }, []);
}
```

The `ControllerSettings` page at `/settings/controller` shows a live visual of all
connected gamepads, their button states, and axis values. Players can rebind any
`GameAction` from the UI; bindings are persisted to `localStorage`.

---

## Native — gilrs

Native desktop builds use **gilrs** for controller input. The SDK provides a thin wrapper
so game code uses the same `GamepadEvent` type regardless of the underlying library.

```rust
// In a native game binary (not WASM):
use magnetite_sdk::input::gamepad::{GamepadState, GamepadEvent};

let mut state = GamepadState::default();
// Your gilrs event loop calls:
state.handle_event(GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, 0.72));
```

---

## FPS example (game-template-fps)

The FPS starter template (`game-template-fps/src/input_map.rs`) uses the `InputMap`
for look (right stick), move (left stick), fire (right trigger), and reload (West button):

```rust
let map = InputMap::default();
// Right stick → Look { dx, dy }  (camera pitch/yaw)
// Left stick  → MoveForward / MoveBackward / MoveLeft / MoveRight
// Right trigger (RT/R2) → Fire
// West (X/Square) → Reload
```

---

## Motorsport example (game-template-motorsport)

The motorsport template reads analog axes for smooth throttle, brake, and steering:

```rust
use magnetite_sdk::input::gamepad::GamepadAxis;

let throttle = state.axis(GamepadAxis::RightTrigger); // 0.0 … 1.0
let brake    = state.axis(GamepadAxis::LeftTrigger);  // 0.0 … 1.0
let steer    = state.axis(GamepadAxis::LeftStickX);   // -1.0 … 1.0
```

---

## See also

- [SDK Reference](./sdk.md) — `input` module types
- [FPS Starter Template](./fps-starter.md)
- [Motorsport Starter Template](./motorsport-starter.md)
- [Graphics Tiers](./graphics-tiers.md)

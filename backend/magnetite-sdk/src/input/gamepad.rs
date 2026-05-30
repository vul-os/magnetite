//! Gamepad / controller input support for Magnetite games.
//!
//! This module provides strongly-typed gamepad state, a comprehensive
//! button/axis enum covering the standard "Xbox-style" layout, and an
//! [`InputMap`] that translates raw hardware events (gamepad *or*
//! keyboard/mouse) into game-level [`GameAction`]s.
//!
//! # Platform binding
//!
//! The types here are **platform-agnostic** — they only model the logical
//! controller layout and the mapping rules.  Integration with the underlying
//! hardware API is done by the platform-specific layer:
//!
//! | Target | Binding |
//! |---|---|
//! | **Browser (WASM)** | [Web Gamepad API](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API) — poll `navigator.getGamepads()` each animation frame and convert to [`GamepadState`] |
//! | **Native (desktop / server)** | [gilrs](https://crates.io/crates/gilrs) — `gilrs::Gilrs::next_event()` in the game loop; enable the `gilrs` feature flag once added to your `Cargo.toml` |
//!
//! The SDK deliberately keeps `gilrs` out of its dependency tree so WASM builds
//! stay light.  A thin adapter crate (`magnetite-gilrs`) bridges the two when
//! building natively — see the documentation for details.
//!
//! # Design
//!
//! The pipeline is:
//!
//! ```text
//! Hardware                    SDK
//!   │                          │
//!   ├─ gamepad button press ──>│ GamepadEvent
//!   ├─ gamepad axis moved ────>│ GamepadEvent
//!   ├─ keyboard key press ────>│ crate::input::InputEvent
//!   └─ mouse delta ───────────>│ crate::input::InputEvent
//!                              │
//!                       InputMap::process_gamepad
//!                       InputMap::process_input
//!                              │
//!                         GameAction
//! ```
//!
//! Games read [`GameAction`]s — not raw buttons or keys — which means
//! remapping is trivial and keyboard/gamepad parity is free.
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::input::gamepad::{
//!     GamepadAxis, GamepadButton, GamepadEvent, GamepadState, InputMap,
//!     InputBinding, GameAction,
//! };
//!
//! // Build a default Xbox-style map.
//! let mut map = InputMap::default();
//!
//! // Simulate pressing the jump button (South / A).
//! let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
//! assert!(actions.contains(&GameAction::Jump));
//!
//! // Query the snapshot directly.
//! let mut state = GamepadState::default();
//! state.apply(&GamepadEvent::ButtonPressed(GamepadButton::North));
//! assert!(state.button(GamepadButton::North));
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Gamepad button enum — standard "Xbox" layout
// ---------------------------------------------------------------------------

/// A physical button on a standard gamepad.
///
/// Follows the naming convention of the Web Gamepad API where possible:
/// face buttons are named by compass direction (South/East/West/North) so the
/// mapping works regardless of whether the physical label says A/B/X/Y or
/// Cross/Circle/Square/Triangle.
///
/// Directional buttons (D-pad) are distinct from analogue axes even though
/// some hardware exposes them as axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    // -- Face buttons --
    /// Bottom face button (Xbox A / PlayStation Cross).
    South,
    /// Right face button (Xbox B / PlayStation Circle).
    East,
    /// Left face button (Xbox X / PlayStation Square).
    West,
    /// Top face button (Xbox Y / PlayStation Triangle).
    North,

    // -- Shoulder / trigger buttons --
    /// Left bumper / shoulder button (LB / L1).
    LeftBumper,
    /// Right bumper / shoulder button (RB / R1).
    RightBumper,
    /// Left trigger as a digital button (LT / L2 — analogue value in [`GamepadAxis::LeftTrigger`]).
    LeftTrigger,
    /// Right trigger as a digital button (RT / R2).
    RightTrigger,

    // -- Stick clicks --
    /// Left stick click (L3).
    LeftStick,
    /// Right stick click (R3).
    RightStick,

    // -- Special buttons --
    /// Select / Back / Share / View.
    Select,
    /// Start / Menu / Options.
    Start,
    /// Guide / Home / PS button.
    Guide,

    // -- D-pad --
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
}

impl GamepadButton {
    /// All standard buttons in index order (matches Web Gamepad API indices 0–16).
    pub const ALL: &'static [GamepadButton] = &[
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::West,
        GamepadButton::North,
        GamepadButton::LeftBumper,
        GamepadButton::RightBumper,
        GamepadButton::LeftTrigger,
        GamepadButton::RightTrigger,
        GamepadButton::Select,
        GamepadButton::Start,
        GamepadButton::LeftStick,
        GamepadButton::RightStick,
        GamepadButton::DPadUp,
        GamepadButton::DPadDown,
        GamepadButton::DPadLeft,
        GamepadButton::DPadRight,
        GamepadButton::Guide,
    ];
}

// ---------------------------------------------------------------------------
// Gamepad axis enum
// ---------------------------------------------------------------------------

/// An analogue axis on a standard gamepad.
///
/// Values range from **-1.0 to 1.0** (for sticks) or **0.0 to 1.0**
/// (for triggers, where 0 = released and 1 = fully pressed).
///
/// The sign convention follows the Web Gamepad API: positive X is right,
/// positive Y is **down** (screen space). Game code that prefers positive Y =
/// up should negate the Y value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadAxis {
    /// Left stick horizontal. Negative = left, positive = right.
    LeftStickX,
    /// Left stick vertical. Negative = up, positive = down (screen-space).
    LeftStickY,
    /// Right stick horizontal. Negative = left, positive = right.
    RightStickX,
    /// Right stick vertical. Negative = up, positive = down (screen-space).
    RightStickY,
    /// Left trigger analogue value (0.0 – 1.0).
    LeftTrigger,
    /// Right trigger analogue value (0.0 – 1.0).
    RightTrigger,
}

impl GamepadAxis {
    /// All standard axes in index order.
    pub const ALL: &'static [GamepadAxis] = &[
        GamepadAxis::LeftStickX,
        GamepadAxis::LeftStickY,
        GamepadAxis::RightStickX,
        GamepadAxis::RightStickY,
        GamepadAxis::LeftTrigger,
        GamepadAxis::RightTrigger,
    ];
}

// ---------------------------------------------------------------------------
// Gamepad events
// ---------------------------------------------------------------------------

/// A single raw event from a gamepad.
///
/// The integration layer (Web Gamepad API polling or gilrs) converts hardware
/// notifications into this enum before passing them to [`GamepadState::apply`]
/// or [`InputMap::process_gamepad`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GamepadEvent {
    /// A button was pressed (digital).
    ButtonPressed(GamepadButton),
    /// A button was released (digital).
    ButtonReleased(GamepadButton),
    /// An analogue axis moved to a new value (-1.0 to 1.0 / 0.0 to 1.0).
    AxisMoved(GamepadAxis, f32),
    /// A gamepad was connected (index is platform-assigned).
    Connected { index: u32 },
    /// A gamepad was disconnected.
    Disconnected { index: u32 },
}

// ---------------------------------------------------------------------------
// Gamepad state snapshot
// ---------------------------------------------------------------------------

/// Snapshot of the complete gamepad state for one frame.
///
/// This captures all buttons and all axes at a single point in time. It is
/// built by accumulating [`GamepadEvent`]s via [`GamepadState::apply`] and
/// included in the per-frame [`crate::input::Input`] sent to the server.
///
/// ```rust
/// use magnetite_sdk::input::gamepad::{GamepadAxis, GamepadButton, GamepadEvent, GamepadState};
///
/// let mut state = GamepadState::default();
/// state.apply(&GamepadEvent::ButtonPressed(GamepadButton::South));
/// state.apply(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, 0.75));
///
/// assert!(state.button(GamepadButton::South));
/// assert!((state.axis(GamepadAxis::LeftStickX) - 0.75).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GamepadState {
    // Buttons stored as a bitmask for compact serialisation.
    // Bit N corresponds to the N-th entry in `GamepadButton::ALL`.
    buttons: u32,

    // Analogue axes — stored in the same order as `GamepadAxis::ALL`.
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_x: f32,
    right_stick_y: f32,
    left_trigger: f32,
    right_trigger: f32,

    /// True when a gamepad is physically connected.
    pub connected: bool,
    /// Platform-assigned gamepad index (0 = first controller).
    pub index: u32,
}

impl GamepadState {
    /// Apply a [`GamepadEvent`] to update this snapshot.
    pub fn apply(&mut self, event: &GamepadEvent) {
        match event {
            GamepadEvent::ButtonPressed(btn) => self.set_button(*btn, true),
            GamepadEvent::ButtonReleased(btn) => self.set_button(*btn, false),
            GamepadEvent::AxisMoved(axis, value) => self.set_axis(*axis, *value),
            GamepadEvent::Connected { index } => {
                self.connected = true;
                self.index = *index;
            }
            GamepadEvent::Disconnected { .. } => {
                self.connected = false;
                // Reset all buttons and axes on disconnect.
                self.buttons = 0;
                self.left_stick_x = 0.0;
                self.left_stick_y = 0.0;
                self.right_stick_x = 0.0;
                self.right_stick_y = 0.0;
                self.left_trigger = 0.0;
                self.right_trigger = 0.0;
            }
        }
    }

    /// Returns `true` if the given button is currently pressed.
    #[inline]
    pub fn button(&self, btn: GamepadButton) -> bool {
        self.buttons & (1 << btn_index(btn)) != 0
    }

    /// Returns the current analogue value of the given axis (-1.0..=1.0 or 0.0..=1.0).
    #[inline]
    pub fn axis(&self, axis: GamepadAxis) -> f32 {
        match axis {
            GamepadAxis::LeftStickX => self.left_stick_x,
            GamepadAxis::LeftStickY => self.left_stick_y,
            GamepadAxis::RightStickX => self.right_stick_x,
            GamepadAxis::RightStickY => self.right_stick_y,
            GamepadAxis::LeftTrigger => self.left_trigger,
            GamepadAxis::RightTrigger => self.right_trigger,
        }
    }

    /// Returns `true` if any button is currently pressed.
    #[inline]
    pub fn any_button(&self) -> bool {
        self.buttons != 0
    }

    /// Returns `true` if any face button (South/East/West/North) is pressed.
    #[inline]
    pub fn any_face_button(&self) -> bool {
        self.button(GamepadButton::South)
            || self.button(GamepadButton::East)
            || self.button(GamepadButton::West)
            || self.button(GamepadButton::North)
    }

    fn set_button(&mut self, btn: GamepadButton, pressed: bool) {
        let mask = 1u32 << btn_index(btn);
        if pressed {
            self.buttons |= mask;
        } else {
            self.buttons &= !mask;
        }
    }

    fn set_axis(&mut self, axis: GamepadAxis, value: f32) {
        let clamped = value.clamp(-1.0, 1.0);
        match axis {
            GamepadAxis::LeftStickX => self.left_stick_x = clamped,
            GamepadAxis::LeftStickY => self.left_stick_y = clamped,
            GamepadAxis::RightStickX => self.right_stick_x = clamped,
            GamepadAxis::RightStickY => self.right_stick_y = clamped,
            GamepadAxis::LeftTrigger => self.left_trigger = value.clamp(0.0, 1.0),
            GamepadAxis::RightTrigger => self.right_trigger = value.clamp(0.0, 1.0),
        }
    }
}

/// Map a [`GamepadButton`] to its bitmask index.
fn btn_index(btn: GamepadButton) -> u32 {
    GamepadButton::ALL
        .iter()
        .position(|b| *b == btn)
        .expect("button not in ALL array") as u32
}

// ---------------------------------------------------------------------------
// Game action enum (intent layer)
// ---------------------------------------------------------------------------

/// A high-level game intent produced by the input mapping layer.
///
/// Games should match on [`GameAction`]s rather than raw keys/buttons so
/// that remapping and platform parity are free.
///
/// ```rust
/// use magnetite_sdk::input::gamepad::GameAction;
///
/// let action = GameAction::Jump;
/// assert!(matches!(action, GameAction::Jump));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameAction {
    /// Move forward.
    MoveForward,
    /// Move backward.
    MoveBackward,
    /// Strafe left.
    MoveLeft,
    /// Strafe right.
    MoveRight,
    /// Look/aim — analogue (dx, dy) in normalised -1..1 space.
    Look { dx: f32, dy: f32 },
    /// Jump.
    Jump,
    /// Crouch / slide.
    Crouch,
    /// Primary fire / interact.
    Fire,
    /// Secondary fire / aim-down-sights.
    AimDownSights,
    /// Reload / use.
    Reload,
    /// Sprint modifier.
    Sprint,
    /// Open pause / menu.
    Pause,
    /// Accelerate (motorsport / driving games).
    Accelerate,
    /// Brake / reverse (motorsport / driving games).
    Brake,
    /// Steer — analogue value (-1.0 = full left, 1.0 = full right).
    Steer(f32),
    /// Handbrake / drift.
    Handbrake,
    /// A game-specific action identified by a string key.
    Custom(String),
}

// ---------------------------------------------------------------------------
// Input binding — a single trigger → action mapping
// ---------------------------------------------------------------------------

/// The source of an input binding: a gamepad button, axis threshold, or
/// keyboard key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputSource {
    /// A digital gamepad button.
    Gamepad(GamepadButton),
    /// An analogue axis exceeding a threshold magnitude (for directional buttons).
    GamepadAxisPositive(GamepadAxis),
    /// An analogue axis below the negative threshold.
    GamepadAxisNegative(GamepadAxis),
    /// A keyboard key (uses the [`crate::input::KeyCode`] names).
    Keyboard(crate::input::KeyCode),
}

/// One binding: when `source` triggers, emit `action`.
///
/// ```rust
/// use magnetite_sdk::input::gamepad::{GameAction, GamepadButton, InputBinding, InputSource};
///
/// let binding = InputBinding {
///     source: InputSource::Gamepad(GamepadButton::South),
///     action: GameAction::Jump,
/// };
/// assert_eq!(binding.action, GameAction::Jump);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBinding {
    /// The hardware source that triggers this binding.
    pub source: InputSource,
    /// The game action to emit.
    pub action: GameAction,
}

// ---------------------------------------------------------------------------
// InputMap — the binding registry
// ---------------------------------------------------------------------------

/// Maps raw hardware inputs (gamepad + keyboard) to [`GameAction`]s.
///
/// Games load an `InputMap` at startup (from a config file or using
/// [`InputMap::default`]) and call [`InputMap::process_gamepad`] /
/// [`InputMap::process_input`] each frame to get the list of active actions.
///
/// # Remapping
///
/// Replace individual entries in [`InputMap::bindings`] or call
/// [`InputMap::bind`] to override defaults at runtime.
///
/// # Default layout (Xbox-style)
///
/// | Source | Action |
/// |---|---|
/// | South (A) | Jump |
/// | East (B) | Crouch |
/// | West (X) | Reload |
/// | North (Y) | AimDownSights |
/// | Left Bumper | Sprint |
/// | Right Bumper | Fire |
/// | Left Trigger | Brake |
/// | Right Trigger | Accelerate |
/// | Left Stick +Y | MoveForward |
/// | Left Stick -Y | MoveBackward |
/// | Left Stick +X | MoveRight |
/// | Left Stick -X | MoveLeft |
/// | Start | Pause |
/// | D-Pad Up | MoveForward (menu / alt) |
/// | D-Pad Down | MoveBackward |
/// | D-Pad Left | MoveLeft |
/// | D-Pad Right | MoveRight |
/// | Keyboard Forward | MoveForward |
/// | Keyboard Backward | MoveBackward |
/// | Keyboard Left | MoveLeft |
/// | Keyboard Right | MoveRight |
/// | Keyboard Jump | Jump |
/// | Keyboard Crouch | Crouch |
/// | Keyboard Attack | Fire |
/// | Keyboard Sprint | Sprint |
/// | Keyboard Escape | Pause |
///
/// The analogue right stick is handled specially: [`InputMap::process_gamepad`]
/// reads the raw axis values and emits a [`GameAction::Look`] action with the
/// current (dx, dy).
///
/// ```rust
/// use magnetite_sdk::input::gamepad::{GameAction, GamepadButton, GamepadEvent, InputMap};
///
/// let mut map = InputMap::default();
/// let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
/// assert!(actions.contains(&GameAction::Jump));
///
/// // Rebind South → Crouch.
/// use magnetite_sdk::input::gamepad::{InputBinding, InputSource};
/// map.bind(InputBinding {
///     source: InputSource::Gamepad(GamepadButton::South),
///     action: GameAction::Crouch,
/// });
/// let actions2 = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
/// assert!(actions2.contains(&GameAction::Crouch));
/// assert!(!actions2.contains(&GameAction::Jump));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMap {
    /// The ordered list of bindings. Later entries override earlier ones for
    /// the same source (the last win).
    pub bindings: Vec<InputBinding>,

    /// Dead-zone for analogue sticks (0.0–1.0). Axis values below this magnitude
    /// are treated as zero. Default: 0.12.
    pub dead_zone: f32,
}

impl Default for InputMap {
    fn default() -> Self {
        use crate::input::KeyCode;
        use GameAction::*;
        use GamepadAxis as Axis;
        use GamepadButton as Btn;
        use InputSource::*;

        let bindings = vec![
            // Face buttons
            InputBinding {
                source: Gamepad(Btn::South),
                action: Jump,
            },
            InputBinding {
                source: Gamepad(Btn::East),
                action: Crouch,
            },
            InputBinding {
                source: Gamepad(Btn::West),
                action: Reload,
            },
            InputBinding {
                source: Gamepad(Btn::North),
                action: AimDownSights,
            },
            // Shoulders / triggers
            InputBinding {
                source: Gamepad(Btn::LeftBumper),
                action: Sprint,
            },
            InputBinding {
                source: Gamepad(Btn::RightBumper),
                action: Fire,
            },
            InputBinding {
                source: GamepadAxisPositive(Axis::RightTrigger),
                action: Accelerate,
            },
            InputBinding {
                source: GamepadAxisPositive(Axis::LeftTrigger),
                action: Brake,
            },
            // Left stick as digital directions (above dead-zone)
            InputBinding {
                source: GamepadAxisNegative(Axis::LeftStickY),
                action: MoveForward,
            },
            InputBinding {
                source: GamepadAxisPositive(Axis::LeftStickY),
                action: MoveBackward,
            },
            InputBinding {
                source: GamepadAxisPositive(Axis::LeftStickX),
                action: MoveRight,
            },
            InputBinding {
                source: GamepadAxisNegative(Axis::LeftStickX),
                action: MoveLeft,
            },
            // Special buttons
            InputBinding {
                source: Gamepad(Btn::Start),
                action: Pause,
            },
            // D-Pad
            InputBinding {
                source: Gamepad(Btn::DPadUp),
                action: MoveForward,
            },
            InputBinding {
                source: Gamepad(Btn::DPadDown),
                action: MoveBackward,
            },
            InputBinding {
                source: Gamepad(Btn::DPadLeft),
                action: MoveLeft,
            },
            InputBinding {
                source: Gamepad(Btn::DPadRight),
                action: MoveRight,
            },
            // Handbrake
            InputBinding {
                source: Gamepad(Btn::LeftStick),
                action: Handbrake,
            },
            // Keyboard equivalents
            InputBinding {
                source: Keyboard(KeyCode::Forward),
                action: MoveForward,
            },
            InputBinding {
                source: Keyboard(KeyCode::Backward),
                action: MoveBackward,
            },
            InputBinding {
                source: Keyboard(KeyCode::Left),
                action: MoveLeft,
            },
            InputBinding {
                source: Keyboard(KeyCode::Right),
                action: MoveRight,
            },
            InputBinding {
                source: Keyboard(KeyCode::Jump),
                action: Jump,
            },
            InputBinding {
                source: Keyboard(KeyCode::Crouch),
                action: Crouch,
            },
            InputBinding {
                source: Keyboard(KeyCode::Attack),
                action: Fire,
            },
            InputBinding {
                source: Keyboard(KeyCode::SecondaryAttack),
                action: AimDownSights,
            },
            InputBinding {
                source: Keyboard(KeyCode::Interact),
                action: Reload,
            },
            InputBinding {
                source: Keyboard(KeyCode::Sprint),
                action: Sprint,
            },
            InputBinding {
                source: Keyboard(KeyCode::Escape),
                action: Pause,
            },
        ];

        Self {
            bindings,
            dead_zone: 0.12,
        }
    }
}

impl InputMap {
    /// Create an `InputMap` with no bindings.
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            dead_zone: 0.12,
        }
    }

    /// Add or override a single binding.
    ///
    /// If a binding for the same source already exists it is replaced; otherwise
    /// the new binding is appended.
    pub fn bind(&mut self, binding: InputBinding) {
        if let Some(existing) = self
            .bindings
            .iter_mut()
            .find(|b| b.source == binding.source)
        {
            existing.action = binding.action;
        } else {
            self.bindings.push(binding);
        }
    }

    /// Remove all bindings for a given source.
    pub fn unbind(&mut self, source: &InputSource) {
        self.bindings.retain(|b| &b.source != source);
    }

    /// Process a single [`GamepadEvent`] and return the matching
    /// [`GameAction`]s.
    ///
    /// Analogue right-stick movement always produces a [`GameAction::Look`] action
    /// regardless of bindings, using the raw (clamped) axis values.
    ///
    /// Only `ButtonPressed` and `AxisMoved` events produce actions; release
    /// events and connect/disconnect events return an empty vec.
    pub fn process_gamepad(&self, event: &GamepadEvent) -> Vec<GameAction> {
        let mut actions = Vec::new();

        match event {
            GamepadEvent::ButtonPressed(btn) => {
                for binding in &self.bindings {
                    if binding.source == InputSource::Gamepad(*btn) {
                        actions.push(binding.action.clone());
                    }
                }
            }
            GamepadEvent::AxisMoved(axis, raw_value) => {
                // Right stick → always Look action (both axes combined in caller).
                if matches!(axis, GamepadAxis::RightStickX | GamepadAxis::RightStickY) {
                    let value = if raw_value.abs() >= self.dead_zone {
                        *raw_value
                    } else {
                        0.0
                    };
                    match axis {
                        GamepadAxis::RightStickX => {
                            actions.push(GameAction::Look { dx: value, dy: 0.0 })
                        }
                        GamepadAxis::RightStickY => {
                            actions.push(GameAction::Look { dx: 0.0, dy: value })
                        }
                        _ => {}
                    }
                    return actions;
                }

                // Left stick → directional actions via dead-zone threshold.
                let value = *raw_value;
                if value.abs() < self.dead_zone {
                    return actions;
                }

                let positive = value > 0.0;
                for binding in &self.bindings {
                    let matches = match &binding.source {
                        InputSource::GamepadAxisPositive(a) => *a == *axis && positive,
                        InputSource::GamepadAxisNegative(a) => *a == *axis && !positive,
                        _ => false,
                    };
                    if matches {
                        actions.push(binding.action.clone());
                    }
                }

                // Steer action for driving games: use left-stick X.
                if *axis == GamepadAxis::LeftStickX {
                    actions.push(GameAction::Steer(value));
                }

                // Trigger actions for driving games: emit Accelerate/Brake analogue.
                if *axis == GamepadAxis::RightTrigger && value > self.dead_zone {
                    if !actions.contains(&GameAction::Accelerate) {
                        actions.push(GameAction::Accelerate);
                    }
                }
                if *axis == GamepadAxis::LeftTrigger && value > self.dead_zone {
                    if !actions.contains(&GameAction::Brake) {
                        actions.push(GameAction::Brake);
                    }
                }
            }
            // Release/connect/disconnect events do not produce positive actions.
            _ => {}
        }

        actions
    }

    /// Process a keyboard/mouse [`crate::input::InputEvent`] and return the
    /// matching [`GameAction`]s.
    ///
    /// Only `Press` events are mapped (releases are intentionally silent so
    /// the game's held-key logic owns that state).
    pub fn process_input(&self, event: &crate::input::InputEvent) -> Vec<GameAction> {
        let mut actions = Vec::new();

        if let crate::input::InputEvent::Press(key) = event {
            for binding in &self.bindings {
                if let InputSource::Keyboard(k) = &binding.source {
                    if k == key {
                        actions.push(binding.action.clone());
                    }
                }
            }
        }

        actions
    }

    /// Look up the primary [`InputSource`] bound to a [`GameAction`].
    ///
    /// Returns the first binding whose action matches, or `None`.
    pub fn source_for_action(&self, action: &GameAction) -> Option<&InputSource> {
        self.bindings
            .iter()
            .find(|b| &b.action == action)
            .map(|b| &b.source)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{InputEvent, KeyCode};

    fn default_map() -> InputMap {
        InputMap::default()
    }

    // -- GamepadState --

    #[test]
    fn gamepad_state_default_no_buttons() {
        let state = GamepadState::default();
        assert!(!state.button(GamepadButton::South));
        assert!(!state.any_button());
    }

    #[test]
    fn gamepad_state_button_press_release() {
        let mut state = GamepadState::default();
        state.apply(&GamepadEvent::ButtonPressed(GamepadButton::North));
        assert!(state.button(GamepadButton::North));
        assert!(!state.button(GamepadButton::South));
        state.apply(&GamepadEvent::ButtonReleased(GamepadButton::North));
        assert!(!state.button(GamepadButton::North));
    }

    #[test]
    fn gamepad_state_all_buttons() {
        let mut state = GamepadState::default();
        for btn in GamepadButton::ALL {
            state.apply(&GamepadEvent::ButtonPressed(*btn));
            assert!(state.button(*btn), "button {:?} should be pressed", btn);
            state.apply(&GamepadEvent::ButtonReleased(*btn));
            assert!(!state.button(*btn), "button {:?} should be released", btn);
        }
    }

    #[test]
    fn gamepad_state_axis_clamped() {
        let mut state = GamepadState::default();
        // Out of range values are clamped.
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, 5.0));
        assert!((state.axis(GamepadAxis::LeftStickX) - 1.0).abs() < 1e-6);
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, -5.0));
        assert!((state.axis(GamepadAxis::LeftStickX) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn gamepad_state_trigger_clamped_zero_one() {
        let mut state = GamepadState::default();
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::LeftTrigger, 1.5));
        assert!((state.axis(GamepadAxis::LeftTrigger) - 1.0).abs() < 1e-6);
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::RightTrigger, 0.5));
        assert!((state.axis(GamepadAxis::RightTrigger) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn gamepad_state_disconnect_resets() {
        let mut state = GamepadState::default();
        state.apply(&GamepadEvent::ButtonPressed(GamepadButton::South));
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, 0.9));
        state.apply(&GamepadEvent::Disconnected { index: 0 });
        assert!(!state.button(GamepadButton::South));
        assert!((state.axis(GamepadAxis::LeftStickX)).abs() < 1e-6);
        assert!(!state.connected);
    }

    #[test]
    fn gamepad_state_any_face_button() {
        let mut state = GamepadState::default();
        assert!(!state.any_face_button());
        state.apply(&GamepadEvent::ButtonPressed(GamepadButton::West));
        assert!(state.any_face_button());
    }

    #[test]
    fn gamepad_state_serde_roundtrip() {
        let mut state = GamepadState::default();
        state.apply(&GamepadEvent::ButtonPressed(GamepadButton::South));
        state.apply(&GamepadEvent::AxisMoved(GamepadAxis::RightStickX, 0.3));
        let json = serde_json::to_string(&state).unwrap();
        let back: GamepadState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    // -- InputMap::process_gamepad --

    #[test]
    fn input_map_south_maps_to_jump() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
        assert!(actions.contains(&GameAction::Jump));
    }

    #[test]
    fn input_map_east_maps_to_crouch() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::East));
        assert!(actions.contains(&GameAction::Crouch));
    }

    #[test]
    fn input_map_right_bumper_maps_to_fire() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::RightBumper));
        assert!(actions.contains(&GameAction::Fire));
    }

    #[test]
    fn input_map_start_maps_to_pause() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::Start));
        assert!(actions.contains(&GameAction::Pause));
    }

    #[test]
    fn input_map_left_stick_y_negative_maps_to_move_forward() {
        let map = default_map();
        // Negative Y = forward (push stick up on screen).
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickY, -0.9));
        assert!(actions.contains(&GameAction::MoveForward));
    }

    #[test]
    fn input_map_left_stick_y_positive_maps_to_move_backward() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickY, 0.9));
        assert!(actions.contains(&GameAction::MoveBackward));
    }

    #[test]
    fn input_map_left_stick_x_positive_maps_to_move_right_and_steer() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickX, 0.8));
        assert!(actions.contains(&GameAction::MoveRight));
        // Steer action should also be present.
        let has_steer = actions.iter().any(|a| matches!(a, GameAction::Steer(_)));
        assert!(has_steer);
    }

    #[test]
    fn input_map_dead_zone_suppresses_small_axis_values() {
        let map = default_map();
        // Value below dead-zone (0.12) should produce no movement actions.
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::LeftStickY, 0.05));
        assert!(!actions.contains(&GameAction::MoveForward));
        assert!(!actions.contains(&GameAction::MoveBackward));
    }

    #[test]
    fn input_map_right_stick_produces_look_action() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::RightStickX, 0.5));
        let has_look = actions
            .iter()
            .any(|a| matches!(a, GameAction::Look { dx, .. } if *dx > 0.0));
        assert!(has_look, "right stick X should produce Look action");
    }

    #[test]
    fn input_map_right_stick_dead_zone_produces_zero_look() {
        let map = default_map();
        // Very small right stick movement should produce Look(0, 0).
        let actions = map.process_gamepad(&GamepadEvent::AxisMoved(GamepadAxis::RightStickX, 0.05));
        let has_zero_look = actions
            .iter()
            .any(|a| matches!(a, GameAction::Look { dx, .. } if dx.abs() < 1e-6));
        assert!(has_zero_look);
    }

    #[test]
    fn input_map_release_produces_no_actions() {
        let map = default_map();
        let actions = map.process_gamepad(&GamepadEvent::ButtonReleased(GamepadButton::South));
        assert!(actions.is_empty());
    }

    // -- InputMap::process_input (keyboard) --

    #[test]
    fn input_map_keyboard_forward_maps_to_move_forward() {
        let map = default_map();
        let actions = map.process_input(&InputEvent::Press(KeyCode::Forward));
        assert!(actions.contains(&GameAction::MoveForward));
    }

    #[test]
    fn input_map_keyboard_jump_maps_to_jump() {
        let map = default_map();
        let actions = map.process_input(&InputEvent::Press(KeyCode::Jump));
        assert!(actions.contains(&GameAction::Jump));
    }

    #[test]
    fn input_map_keyboard_release_produces_no_actions() {
        let map = default_map();
        let actions = map.process_input(&InputEvent::Release(KeyCode::Forward));
        assert!(actions.is_empty());
    }

    #[test]
    fn input_map_keyboard_escape_maps_to_pause() {
        let map = default_map();
        let actions = map.process_input(&InputEvent::Press(KeyCode::Escape));
        assert!(actions.contains(&GameAction::Pause));
    }

    // -- InputMap::bind / unbind --

    #[test]
    fn input_map_bind_overrides_existing() {
        let mut map = default_map();
        map.bind(InputBinding {
            source: InputSource::Gamepad(GamepadButton::South),
            action: GameAction::Crouch,
        });
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
        assert!(actions.contains(&GameAction::Crouch));
        assert!(!actions.contains(&GameAction::Jump));
    }

    #[test]
    fn input_map_unbind_removes_source() {
        let mut map = default_map();
        map.unbind(&InputSource::Gamepad(GamepadButton::South));
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
        assert!(!actions.contains(&GameAction::Jump));
    }

    #[test]
    fn input_map_source_for_action() {
        let map = default_map();
        let src = map.source_for_action(&GameAction::Jump);
        assert!(src.is_some());
    }

    #[test]
    fn input_map_empty_has_no_bindings() {
        let map = InputMap::empty();
        let actions = map.process_gamepad(&GamepadEvent::ButtonPressed(GamepadButton::South));
        assert!(actions.is_empty());
    }

    #[test]
    fn input_map_serde_roundtrip() {
        let map = default_map();
        let json = serde_json::to_string(&map).unwrap();
        let back: InputMap = serde_json::from_str(&json).unwrap();
        assert_eq!(map.bindings.len(), back.bindings.len());
        assert!((map.dead_zone - back.dead_zone).abs() < 1e-9);
    }

    // -- InputBinding --

    #[test]
    fn input_binding_serde_roundtrip() {
        let binding = InputBinding {
            source: InputSource::Gamepad(GamepadButton::Guide),
            action: GameAction::Custom("overlay".to_string()),
        };
        let json = serde_json::to_string(&binding).unwrap();
        let back: InputBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(binding, back);
    }

    // -- GamepadButton::ALL coverage --

    #[test]
    fn all_gamepad_buttons_fit_in_bitmask() {
        // We use a u32 bitmask; ensure we never exceed 32 buttons.
        assert!(
            GamepadButton::ALL.len() <= 32,
            "too many buttons for u32 bitmask"
        );
    }

    #[test]
    fn gamepad_button_all_unique_indices() {
        // Each button should map to a unique index.
        let mut seen = std::collections::HashSet::new();
        for btn in GamepadButton::ALL {
            let idx = btn_index(*btn);
            assert!(seen.insert(idx), "duplicate index for {:?}", btn);
        }
    }
}

//! Strongly-typed input and action types.
//!
//! Magnetite separates *raw input* (what the hardware reports) from *actions*
//! (what the game logic interprets). This design lets the server re-simulate
//! a sequence of inputs for deterministic replay and client-side prediction.
//!
//! ## Sub-modules
//!
//! | Sub-module | Purpose |
//! |---|---|
//! | [`gamepad`] | Gamepad/controller input — [`gamepad::GamepadState`], buttons, axes, and the [`gamepad::InputMap`] binding layer |
//!
//! # Input pipeline
//!
//! ```text
//! Hardware → InputEvent stream → KeyState/MouseState snapshot → Input frame
//!                                                                     │
//!                                                               GameLogic::handle_input
//!                                                                     │
//!                                                               Action (authoritative)
//! ```
//!
//! # Example
//!
//! ```rust
//! use magnetite_sdk::input::{Input, InputEvent, KeyCode, KeyState, MouseState};
//!
//! // Build a snapshot from a stream of events.
//! let mut ks = KeyState::default();
//! for event in &[InputEvent::Press(KeyCode::Forward), InputEvent::Press(KeyCode::Jump)] {
//!     ks.apply(event);
//! }
//! let frame = Input { keys: ks, mouse: MouseState::default(), sequence: 1, timestamp_ms: 0 };
//! assert!(frame.keys.forward);
//! assert!(frame.keys.jump);
//! ```

pub mod gamepad;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Raw input types
// ---------------------------------------------------------------------------

/// A logical key that can be pressed or released.
///
/// This is intentionally game-agnostic; games with different control schemes
/// should define their own action mapping on top of these primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    /// Primary forward movement (W / ↑).
    Forward,
    /// Primary backward movement (S / ↓).
    Backward,
    /// Strafe left (A / ←).
    Left,
    /// Strafe right (D / →).
    Right,
    /// Jump / ascend (Space).
    Jump,
    /// Crouch / descend (Ctrl / C).
    Crouch,
    /// Primary attack / fire (left-click / Z).
    Attack,
    /// Secondary attack / aim (right-click / X).
    SecondaryAttack,
    /// Reload / interact (R / E).
    Interact,
    /// Sprint modifier (Shift).
    Sprint,
    /// Escape / pause.
    Escape,
    /// An arbitrary game-specific key, identified by an index.
    Custom(u8),
}

/// A single hardware input event.
///
/// Accumulate these into a [`KeyState`] snapshot with [`KeyState::apply`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// A key was pressed.
    Press(KeyCode),
    /// A key was released.
    Release(KeyCode),
    /// Absolute mouse/pointer position (pixels from top-left).
    MouseMove { x: f64, y: f64 },
    /// Relative mouse motion since the last event (pixels).
    MouseDelta { dx: f64, dy: f64 },
    /// Mouse button pressed (`0` = left, `1` = right, `2` = middle).
    MouseButtonPress(u8),
    /// Mouse button released.
    MouseButtonRelease(u8),
    /// Mouse wheel delta (positive = scroll up / zoom in).
    MouseWheel(f64),
}

/// Snapshot of held-key states for a single input frame.
///
/// This is built by accumulating [`InputEvent`]s via [`KeyState::apply`].
/// The snapshot is sent from client to server once per tick.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct KeyState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub crouch: bool,
    pub attack: bool,
    pub secondary_attack: bool,
    pub interact: bool,
    pub sprint: bool,
}

impl KeyState {
    /// Update this snapshot by applying a single [`InputEvent`].
    ///
    /// Unknown events (mouse, wheel) are silently ignored.
    pub fn apply(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Press(k) => self.set(k, true),
            InputEvent::Release(k) => self.set(k, false),
            _ => {}
        }
    }

    fn set(&mut self, key: &KeyCode, pressed: bool) {
        match key {
            KeyCode::Forward => self.forward = pressed,
            KeyCode::Backward => self.backward = pressed,
            KeyCode::Left => self.left = pressed,
            KeyCode::Right => self.right = pressed,
            KeyCode::Jump => self.jump = pressed,
            KeyCode::Crouch => self.crouch = pressed,
            KeyCode::Attack => self.attack = pressed,
            KeyCode::SecondaryAttack => self.secondary_attack = pressed,
            KeyCode::Interact => self.interact = pressed,
            KeyCode::Sprint => self.sprint = pressed,
            KeyCode::Escape | KeyCode::Custom(_) => {}
        }
    }

    /// Returns `true` if any movement key is held.
    #[inline]
    pub fn any_movement(&self) -> bool {
        self.forward || self.backward || self.left || self.right
    }
}

/// Snapshot of the pointing device state for a single input frame.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct MouseState {
    /// Absolute pointer position (viewport pixels, origin top-left).
    pub x: f64,
    pub y: f64,
    /// Relative motion since the previous frame (pixels).
    pub delta_x: f64,
    pub delta_y: f64,
    /// Whether each mouse button is currently held.
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    /// Accumulated scroll delta since the previous frame.
    pub scroll: f64,
}

impl MouseState {
    /// Apply a mouse-related [`InputEvent`] to this snapshot.
    pub fn apply(&mut self, event: &InputEvent) {
        match event {
            InputEvent::MouseMove { x, y } => {
                self.x = *x;
                self.y = *y;
            }
            InputEvent::MouseDelta { dx, dy } => {
                self.delta_x += dx;
                self.delta_y += dy;
            }
            InputEvent::MouseButtonPress(0) => self.left_button = true,
            InputEvent::MouseButtonPress(1) => self.right_button = true,
            InputEvent::MouseButtonPress(2) => self.middle_button = true,
            InputEvent::MouseButtonRelease(0) => self.left_button = false,
            InputEvent::MouseButtonRelease(1) => self.right_button = false,
            InputEvent::MouseButtonRelease(2) => self.middle_button = false,
            InputEvent::MouseWheel(d) => self.scroll += d,
            _ => {}
        }
    }
}

/// A complete, self-contained input frame sent from a client to the server.
///
/// `sequence` monotonically increases per-client; the server uses it to detect
/// dropped frames and to match predictions with authoritative responses.
///
/// ```rust
/// use magnetite_sdk::input::{Input, KeyState, MouseState};
///
/// let frame = Input {
///     keys: KeyState { forward: true, ..Default::default() },
///     mouse: MouseState::default(),
///     sequence: 42,
///     timestamp_ms: 1_000,
/// };
/// assert_eq!(frame.sequence, 42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// Keyboard snapshot for this frame.
    pub keys: KeyState,
    /// Mouse snapshot for this frame.
    pub mouse: MouseState,
    /// Monotonically increasing frame counter (client-local).
    pub sequence: u64,
    /// Client wall-clock time in milliseconds (for latency measurement).
    pub timestamp_ms: u64,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keys: KeyState::default(),
            mouse: MouseState::default(),
            sequence: 0,
            timestamp_ms: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Action types — the output of `GameLogic::handle_input`
// ---------------------------------------------------------------------------

/// A movement direction in the horizontal plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Backward,
    Left,
    Right,
}

/// The authoritative action produced by the game logic after interpreting a
/// single player's [`Input`] frame.
///
/// Actions are what the server records in a replay buffer. The client also
/// predicts actions locally and reconciles against server responses.
///
/// For games with more complex action spaces, use [`Action::Custom`] and
/// serialise your action type into the payload.
///
/// # Example
///
/// ```rust
/// use magnetite_sdk::input::{Action, Direction};
///
/// let action = Action::Move { direction: Direction::Forward, sprint: false };
/// assert!(matches!(action, Action::Move { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// The player moved in `direction`. `sprint` indicates whether the sprint
    /// modifier was active.
    Move { direction: Direction, sprint: bool },
    /// The player jumped.
    Jump,
    /// The player crouched (or toggled crouch).
    Crouch,
    /// Primary attack / fire.
    Attack,
    /// Secondary attack / aim.
    SecondaryAttack,
    /// Interact with an object.
    Interact,
    /// Game-specific action. Serialise the payload with [`serde_json`].
    Custom {
        /// A short identifier string (e.g. `"reload"`, `"ability_1"`).
        name: String,
        /// Arbitrary JSON payload.
        payload: serde_json::Value,
    },
    /// No action this frame (the player was idle).
    None,
}

impl Action {
    /// Returns `true` if no meaningful action occurred.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_state_apply_press_release() {
        let mut ks = KeyState::default();
        ks.apply(&InputEvent::Press(KeyCode::Forward));
        assert!(ks.forward);
        ks.apply(&InputEvent::Release(KeyCode::Forward));
        assert!(!ks.forward);
    }

    #[test]
    fn key_state_any_movement() {
        let mut ks = KeyState::default();
        assert!(!ks.any_movement());
        ks.forward = true;
        assert!(ks.any_movement());
    }

    #[test]
    fn mouse_state_apply() {
        let mut ms = MouseState::default();
        ms.apply(&InputEvent::MouseMove { x: 100.0, y: 200.0 });
        assert_eq!(ms.x, 100.0);
        ms.apply(&InputEvent::MouseDelta { dx: 3.0, dy: -2.0 });
        assert_eq!(ms.delta_x, 3.0);
        ms.apply(&InputEvent::MouseButtonPress(1));
        assert!(ms.right_button);
        ms.apply(&InputEvent::MouseButtonRelease(1));
        assert!(!ms.right_button);
        ms.apply(&InputEvent::MouseWheel(1.5));
        assert!((ms.scroll - 1.5).abs() < 1e-9);
    }

    #[test]
    fn input_serde_roundtrip() {
        let frame = Input {
            keys: KeyState {
                attack: true,
                sprint: true,
                ..Default::default()
            },
            mouse: MouseState {
                delta_x: 1.5,
                ..Default::default()
            },
            sequence: 99,
            timestamp_ms: 12345,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let frame2: Input = serde_json::from_str(&json).unwrap();
        assert_eq!(frame, frame2);
    }

    #[test]
    fn action_is_none() {
        assert!(Action::None.is_none());
        assert!(!Action::Jump.is_none());
    }

    #[test]
    fn action_custom_serde() {
        let action = Action::Custom {
            name: "ability_1".to_string(),
            payload: serde_json::json!({ "target_id": 5 }),
        };
        let json = serde_json::to_string(&action).unwrap();
        let action2: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(action, action2);
    }
}

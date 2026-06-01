//! # FPS Input Map — Unified Keyboard/Mouse + Gamepad Input
//!
//! Magnetite provides a raw [`Input`] frame (keyboard bools + mouse deltas).
//! This module layers a **unified FPS action layer** on top so that:
//!
//! - Keyboard/mouse (`WASD`, mouse delta) and
//! - Gamepad (left stick move, right stick look, triggers for fire/aim)
//!
//! produce the same [`FpsAction`] consumed by `FpsGame::handle_input`.
//!
//! ## Gamepad protocol (native + WASM)
//!
//! **Native** — gilrs reads controller state and injects it into the
//! [`Input`] frame via `KeyCode::Custom` codes:
//!
//! | Custom code | Meaning |
//! |---|---|
//! | `GAMEPAD_LEFT_X` | Left stick X axis (signed i8 scaled to `[-127, 127]`) |
//! | `GAMEPAD_LEFT_Y` | Left stick Y axis |
//! | `GAMEPAD_RIGHT_X` | Right stick X axis (look) |
//! | `GAMEPAD_RIGHT_Y` | Right stick Y axis (look) |
//! | `GAMEPAD_BTN_A` | South button (jump) |
//! | `GAMEPAD_BTN_B` | East button (crouch) |
//! | `GAMEPAD_BTN_X` | West button (reload) |
//! | `GAMEPAD_BTN_RT` | Right trigger (fire) |
//! | `GAMEPAD_BTN_LT` | Left trigger (aim-down-sights) |
//! | `GAMEPAD_BTN_LB` | Left bumper (sprint) |
//! | `GAMEPAD_BTN_MENU` | Menu / pause |
//!
//! **WASM** — the browser Gamepad API polls controller state each frame;
//! the JS host maps it to the same `KeyCode::Custom` codes before calling
//! `FpsGameHandle::handle_input`.
//!
//! This design keeps the server-side `FpsGame` completely platform-agnostic —
//! it only ever sees a canonical [`Input`] frame regardless of the device.

use magnetite_sdk::input::{Action, Direction, Input};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Gamepad key-code constants (Custom byte codes)
// ---------------------------------------------------------------------------

/// Left stick horizontal axis (value encoded in the `Custom(u8)` press/release
/// events as a scaled integer — see the gilrs integration for the encoder).
pub const GAMEPAD_LEFT_X: u8 = 0x10;
pub const GAMEPAD_LEFT_Y: u8 = 0x11;

/// Right stick horizontal / vertical (camera look).
pub const GAMEPAD_RIGHT_X: u8 = 0x12;
pub const GAMEPAD_RIGHT_Y: u8 = 0x13;

/// Face buttons.
pub const GAMEPAD_BTN_A: u8 = 0x20; // Jump
pub const GAMEPAD_BTN_B: u8 = 0x21; // Crouch
pub const GAMEPAD_BTN_X: u8 = 0x22; // Reload
pub const GAMEPAD_BTN_Y: u8 = 0x23; // Interact

/// Triggers and bumpers.
pub const GAMEPAD_BTN_RT: u8 = 0x30; // Fire (right trigger)
pub const GAMEPAD_BTN_LT: u8 = 0x31; // Aim down sights (left trigger)
pub const GAMEPAD_BTN_RB: u8 = 0x32; // Weapon swap
pub const GAMEPAD_BTN_LB: u8 = 0x33; // Sprint

/// System.
pub const GAMEPAD_BTN_MENU: u8 = 0x40; // Pause / menu

// ---------------------------------------------------------------------------
// Gamepad axis / button type aliases (mirroring what gilrs provides)
// ---------------------------------------------------------------------------

/// Named gamepad axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
}

/// Named gamepad buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadButton {
    South, // A (Xbox), Cross (PlayStation)
    East,  // B, Circle
    West,  // X, Square
    North, // Y, Triangle
    LeftTrigger,
    RightTrigger,
    LeftBumper,
    RightBumper,
    Start,
}

// ---------------------------------------------------------------------------
// FPS-level action enum
// ---------------------------------------------------------------------------

/// The high-level action produced by [`InputMap::resolve`].
///
/// This sits between the raw [`Input`] frame and the game logic: it is
/// independent of whether the action came from a keyboard, mouse, or gamepad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FpsAction {
    /// Move forward; `sprinting` when the sprint key/button is held.
    MoveForward {
        sprinting: bool,
    },
    MoveBackward,
    MoveLeft,
    MoveRight,
    /// Analog stick movement: `x` ∈ [-1, 1] (strafe), `z` ∈ [-1, 1] (forward).
    MoveAnalog {
        x: f32,
        z: f32,
        sprinting: bool,
    },
    Jump,
    Crouch,
    /// Primary fire.
    Fire,
    /// Reload the current weapon.
    Reload,
    /// Aim-down-sights toggle.
    Aim,
    Interact,
    None,
}

impl FpsAction {
    /// Convert to the SDK [`Action`] type for the replay/netcode log.
    ///
    /// The SDK only knows about generic actions; game-specific ones use
    /// `Action::Custom` with a JSON payload.
    pub fn into_sdk_action(self) -> Action {
        match self {
            FpsAction::MoveForward { sprinting } => Action::Move {
                direction: Direction::Forward,
                sprint: sprinting,
            },
            FpsAction::MoveBackward => Action::Move {
                direction: Direction::Backward,
                sprint: false,
            },
            FpsAction::MoveLeft => Action::Move {
                direction: Direction::Left,
                sprint: false,
            },
            FpsAction::MoveRight => Action::Move {
                direction: Direction::Right,
                sprint: false,
            },
            FpsAction::MoveAnalog { x, z, sprinting } => Action::Custom {
                name: "move_analog".into(),
                payload: serde_json::json!({ "x": x, "z": z, "sprint": sprinting }),
            },
            FpsAction::Jump => Action::Jump,
            FpsAction::Crouch => Action::Crouch,
            FpsAction::Fire => Action::Attack,
            FpsAction::Reload => Action::Custom {
                name: "reload".into(),
                payload: serde_json::Value::Null,
            },
            FpsAction::Aim => Action::SecondaryAttack,
            FpsAction::Interact => Action::Interact,
            FpsAction::None => Action::None,
        }
    }
}

// ---------------------------------------------------------------------------
// InputMap — the translator
// ---------------------------------------------------------------------------

/// Translates a raw Magnetite [`Input`] frame into an [`FpsAction`].
///
/// Priority order (highest to lowest):
/// 1. Gamepad analog stick movement (if non-zero magnitude)
/// 2. Gamepad digital button presses
/// 3. Keyboard + mouse
pub struct InputMap;

impl InputMap {
    /// Resolve an [`Input`] frame to the dominant [`FpsAction`] for this tick.
    ///
    /// Note: **look** (yaw/pitch) from mouse and right-stick is applied
    /// directly in `FpsGame::handle_input` from `input.mouse.delta_x/y`.
    /// This function only handles *movement and action* resolution.
    pub fn resolve(input: &Input) -> FpsAction {
        let keys = &input.keys;
        let sprinting = keys.sprint || Self::gamepad_button(input, GAMEPAD_BTN_LB);

        // ── 1. Analog stick movement ─────────────────────────────────────────
        let lx = Self::analog_axis(input, GAMEPAD_LEFT_X);
        let ly = Self::analog_axis(input, GAMEPAD_LEFT_Y);
        // Dead-zone: ignore tiny stick deflections.
        if lx.hypot(ly) > 0.15 {
            return FpsAction::MoveAnalog {
                x: lx,
                z: ly,
                sprinting,
            };
        }

        // ── 2. Digital buttons / keys ────────────────────────────────────────
        // Fire (right trigger or left mouse button).
        if keys.attack || Self::gamepad_button(input, GAMEPAD_BTN_RT) {
            return FpsAction::Fire;
        }

        // Aim-down-sights (left trigger or right mouse button).
        if keys.secondary_attack || Self::gamepad_button(input, GAMEPAD_BTN_LT) {
            return FpsAction::Aim;
        }

        // Reload (R key mapped to Custom(GAMEPAD_BTN_X) or secondary_attack).
        // We map `secondary_attack` to reload here only when ADS is not pressed —
        // the above branch handles ADS first, so we're safe.
        // Reload is triggered by the X/West button on a gamepad.
        if Self::gamepad_button(input, GAMEPAD_BTN_X) {
            return FpsAction::Reload;
        }

        // Jump.
        if keys.jump || Self::gamepad_button(input, GAMEPAD_BTN_A) {
            return FpsAction::Jump;
        }

        // Crouch.
        if keys.crouch || Self::gamepad_button(input, GAMEPAD_BTN_B) {
            return FpsAction::Crouch;
        }

        // Interact.
        if keys.interact || Self::gamepad_button(input, GAMEPAD_BTN_Y) {
            return FpsAction::Interact;
        }

        // ── 3. WASD movement ─────────────────────────────────────────────────
        if keys.forward {
            return FpsAction::MoveForward { sprinting };
        }
        if keys.backward {
            return FpsAction::MoveBackward;
        }
        if keys.left {
            return FpsAction::MoveLeft;
        }
        if keys.right {
            return FpsAction::MoveRight;
        }

        FpsAction::None
    }

    /// Read an analog axis value from `KeyCode::Custom` press events.
    ///
    /// The gilrs Bevy system (and the JS host for WASM) encodes axis values as
    /// a `Custom(axis_code)` key *press* event where the axis value is baked
    /// into the mouse `delta_x` / `delta_y` channel (right stick) or queued
    /// as a separate analog field. For simplicity in the SDK-only path we use
    /// the mouse delta channels as the analog axis transport:
    ///
    /// - `GAMEPAD_LEFT_X` / `GAMEPAD_LEFT_Y` → `mouse.delta_x` / `mouse.delta_y`
    ///   (overridden by the gilrs system; keyboard path leaves these at 0).
    /// - `GAMEPAD_RIGHT_X` / `GAMEPAD_RIGHT_Y` → handled in the look system.
    ///
    /// This returns a normalised value in `[-1.0, 1.0]`.
    fn analog_axis(input: &Input, axis_code: u8) -> f32 {
        match axis_code {
            // Left stick X — strafe.
            // The gilrs integration writes left-stick into mouse.delta_x
            // only when no mouse device is active. For the SDK path we
            // just use 0 (pure keyboard) unless the gilrs system filled it.
            GAMEPAD_LEFT_X => {
                // Check for a very large delta that could only come from a
                // gamepad (mouse deltas are typically < 100 px per frame).
                let raw = input.mouse.delta_x as f32;
                // Scale: gilrs writes values in [-32768, 32767]; normalise.
                let v = if raw.abs() > 1.0 { raw / 32768.0 } else { 0.0 };
                v.clamp(-1.0, 1.0)
            }
            GAMEPAD_LEFT_Y => {
                let raw = input.mouse.delta_y as f32;
                let v = if raw.abs() > 1.0 { raw / 32768.0 } else { 0.0 };
                v.clamp(-1.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// Check whether a gamepad digital button is pressed.
    ///
    /// The Bevy `collect_gamepad_input` system (native) and the WASM JS host
    /// both map controller buttons onto the named [`KeyState`] boolean fields
    /// before the `Input` frame reaches this function.  This function decodes
    /// that mapping so that the logical `GAMEPAD_BTN_*` constants stay in sync
    /// with what the integrations write:
    ///
    /// | Button constant | [`KeyState`] field |
    /// |---|---|
    /// | `GAMEPAD_BTN_A` (South / Jump) | `keys.jump` |
    /// | `GAMEPAD_BTN_B` (East / Crouch) | `keys.crouch` |
    /// | `GAMEPAD_BTN_X` (West / Reload) | `keys.interact` |
    /// | `GAMEPAD_BTN_Y` (North / Interact) | `keys.interact` |
    /// | `GAMEPAD_BTN_RT` (Right trigger / Fire) | `keys.attack` |
    /// | `GAMEPAD_BTN_LT` (Left trigger / Aim) | `keys.secondary_attack` |
    /// | `GAMEPAD_BTN_LB` (Left bumper / Sprint) | `keys.sprint` |
    ///
    /// Buttons without a named `KeyState` field (e.g. RB, Menu) return
    /// `false` until the SDK adds a `custom_buttons` map.
    fn gamepad_button(input: &Input, btn: u8) -> bool {
        match btn {
            GAMEPAD_BTN_A => input.keys.jump,
            GAMEPAD_BTN_B => input.keys.crouch,
            // West (X) is bound to Reload; the Bevy integration writes it
            // into `keys.interact` (shared with the keyboard Interact key).
            GAMEPAD_BTN_X | GAMEPAD_BTN_Y => input.keys.interact,
            GAMEPAD_BTN_RT => input.keys.attack,
            GAMEPAD_BTN_LT => input.keys.secondary_attack,
            GAMEPAD_BTN_LB => input.keys.sprint,
            // RB, Menu, and other buttons have no named KeyState field yet.
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use magnetite_sdk::input::{Input, KeyState, MouseState};

    fn make(keys: KeyState) -> Input {
        Input {
            keys,
            mouse: MouseState::default(),
            sequence: 1,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn forward_key_resolves_to_move_forward() {
        let input = make(KeyState {
            forward: true,
            ..Default::default()
        });
        assert!(matches!(
            InputMap::resolve(&input),
            FpsAction::MoveForward { sprinting: false }
        ));
    }

    #[test]
    fn sprint_modifier_sets_sprinting() {
        let input = make(KeyState {
            forward: true,
            sprint: true,
            ..Default::default()
        });
        assert!(matches!(
            InputMap::resolve(&input),
            FpsAction::MoveForward { sprinting: true }
        ));
    }

    #[test]
    fn backward_key_resolves_correctly() {
        let input = make(KeyState {
            backward: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::MoveBackward);
    }

    #[test]
    fn attack_key_resolves_to_fire() {
        let input = make(KeyState {
            attack: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::Fire);
    }

    #[test]
    fn secondary_attack_resolves_to_aim() {
        let input = make(KeyState {
            secondary_attack: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::Aim);
    }

    #[test]
    fn jump_key_resolves_to_jump() {
        let input = make(KeyState {
            jump: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::Jump);
    }

    #[test]
    fn crouch_key_resolves_to_crouch() {
        let input = make(KeyState {
            crouch: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::Crouch);
    }

    #[test]
    fn interact_key_resolves_to_interact() {
        let input = make(KeyState {
            interact: true,
            ..Default::default()
        });
        assert_eq!(InputMap::resolve(&input), FpsAction::Interact);
    }

    #[test]
    fn no_input_resolves_to_none() {
        let input = make(KeyState::default());
        assert_eq!(InputMap::resolve(&input), FpsAction::None);
    }

    #[test]
    fn fps_action_into_sdk_action_fire() {
        let sdk = FpsAction::Fire.into_sdk_action();
        assert!(matches!(sdk, Action::Attack));
    }

    #[test]
    fn fps_action_into_sdk_action_move_analog() {
        let sdk = FpsAction::MoveAnalog {
            x: 0.5,
            z: 0.8,
            sprinting: false,
        }
        .into_sdk_action();
        assert!(matches!(sdk, Action::Custom { ref name, .. } if name == "move_analog"));
    }
}

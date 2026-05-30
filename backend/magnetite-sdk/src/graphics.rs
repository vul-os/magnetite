//! Graphics / engine tiers for Magnetite games.
//!
//! Every Magnetite game declares a [`GraphicsTier`] in its [`RenderConfig`].
//! The platform uses this declaration to:
//!
//! 1. **Choose an appropriate runtime** — WASM builds for [`GraphicsTier::Lite2D`]
//!    run in a lightweight canvas2D context; [`GraphicsTier::Advanced3D`] targets
//!    WebGPU / native Vulkan via Bevy.
//! 2. **Allocate server resources** — physics/simulation budget scales with the tier.
//! 3. **Surface correct capabilities to the SDK** — tier-specific features
//!    (e.g. physics joints, post-processing) are validated at startup.
//!
//! # How a game declares its tier
//!
//! Return a [`RenderConfig`] from the game's [`crate::game::GameLogic`] metadata
//! or embed it in [`crate::game::GameMetadata`] (the platform reads it on load):
//!
//! ```rust
//! use magnetite_sdk::graphics::{GraphicsTier, RenderConfig};
//!
//! // A simple 2D arcade game.
//! let simple = RenderConfig::new(GraphicsTier::Lite2D);
//!
//! // An FPS with full 3D graphics.
//! let fps = RenderConfig::builder()
//!     .tier(GraphicsTier::Standard3D)
//!     .target_fps(60)
//!     .shadow_quality(2)
//!     .hdr(false)
//!     .build();
//!
//! // A motorsport title with advanced visuals.
//! let motorsport = RenderConfig::builder()
//!     .tier(GraphicsTier::Advanced3D)
//!     .target_fps(120)
//!     .shadow_quality(4)
//!     .hdr(true)
//!     .post_processing(true)
//!     .physics_substeps(8)
//!     .build();
//!
//! assert_eq!(simple.tier, GraphicsTier::Lite2D);
//! assert_eq!(motorsport.physics_substeps, 8);
//! ```
//!
//! # Tier comparison
//!
//! | Property | `Lite2D` | `Standard3D` | `Advanced3D` |
//! |---|---|---|---|
//! | Renderer | Canvas 2D / WebGL | WebGL2 / WebGPU | WebGPU / Vulkan / Metal |
//! | Physics engine | none / simple AABB | rapier2d or rapier3d (basic) | rapier3d (full joints + substeps) |
//! | Shadow quality | none | medium (2) | high (4) |
//! | HDR / post-FX | no | optional | yes |
//! | Max target FPS | 60 | 120 | 240 |
//! | Typical use | 2D arcade / jam games | 3D indie / FPS starter | AAA FPS / motorsport |
//!
//! # Engine capability checks
//!
//! [`RenderConfig::supports`] lets game code assert at startup that the host
//! platform can satisfy its requirements:
//!
//! ```rust
//! use magnetite_sdk::graphics::{GraphicsTier, RenderConfig, EngineCapability};
//!
//! let config = RenderConfig::builder()
//!     .tier(GraphicsTier::Advanced3D)
//!     .hdr(true)
//!     .build();
//!
//! assert!(config.supports(EngineCapability::Hdr));
//! assert!(config.supports(EngineCapability::PostProcessing));
//! assert!(!config.supports(EngineCapability::Canvas2D));
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// GraphicsTier
// ---------------------------------------------------------------------------

/// The graphics/engine tier a game targets.
///
/// Declare the tier in [`RenderConfig`] and pass it to the Magnetite platform
/// on game load. The platform uses it to provision the correct runtime
/// environment.
///
/// ```rust
/// use magnetite_sdk::graphics::GraphicsTier;
///
/// let tier = GraphicsTier::Standard3D;
/// let json = serde_json::to_string(&tier).unwrap();
/// let back: GraphicsTier = serde_json::from_str(&json).unwrap();
/// assert_eq!(tier, back);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsTier {
    /// **Lightweight 2D.**
    ///
    /// Intended for jam games, card games, turn-based, or simple arcade titles.
    ///
    /// - Canvas 2D / WebGL sprite renderer (Bevy optional, or plain web canvas).
    /// - No real-time 3D lighting or shadows.
    /// - Very fast WASM build — minimal Bevy feature set.
    /// - No GPU physics; simple AABB collision optional.
    Lite2D,

    /// **3D — standard quality.**
    ///
    /// Covers most 3D indie titles, FPS starters, and third-person action games.
    ///
    /// - WebGL2 or WebGPU renderer (Bevy `DefaultPlugins` + `bevy_rapier3d`).
    /// - Real-time directional + point lighting, medium shadow maps.
    /// - 60–120 FPS target.
    /// - rapier3d (basic joint support).
    Standard3D,

    /// **3D — advanced quality.**
    ///
    /// For AAA-scale FPS or complex motorsport / racing simulations.
    ///
    /// - WebGPU (browser) / Vulkan or Metal (native) via Bevy.
    /// - HDR, PBR materials, bloom, depth-of-field, motion blur.
    /// - High-quality cascaded shadow maps.
    /// - rapier3d with multiple physics substeps (8+) for vehicle simulation.
    /// - 120–240 FPS target (native).
    Advanced3D,
}

impl GraphicsTier {
    /// Returns `true` if this tier is at least as capable as `other`.
    ///
    /// ```rust
    /// use magnetite_sdk::graphics::GraphicsTier;
    ///
    /// assert!(GraphicsTier::Advanced3D.at_least(GraphicsTier::Standard3D));
    /// assert!(!GraphicsTier::Lite2D.at_least(GraphicsTier::Standard3D));
    /// ```
    #[inline]
    pub fn at_least(&self, other: GraphicsTier) -> bool {
        *self >= other
    }

    /// Human-readable label for the tier.
    pub fn label(&self) -> &'static str {
        match self {
            GraphicsTier::Lite2D => "Lite 2D",
            GraphicsTier::Standard3D => "Standard 3D",
            GraphicsTier::Advanced3D => "Advanced 3D",
        }
    }

    /// Suggested maximum target FPS for each tier.
    pub fn max_target_fps(&self) -> u32 {
        match self {
            GraphicsTier::Lite2D => 60,
            GraphicsTier::Standard3D => 120,
            GraphicsTier::Advanced3D => 240,
        }
    }

    /// Recommended physics substep count for each tier.
    pub fn recommended_physics_substeps(&self) -> u32 {
        match self {
            GraphicsTier::Lite2D => 1,
            GraphicsTier::Standard3D => 4,
            GraphicsTier::Advanced3D => 8,
        }
    }
}

// ---------------------------------------------------------------------------
// EngineCapability — for compile-time / runtime assertions
// ---------------------------------------------------------------------------

/// A specific engine capability that a game may require.
///
/// Pass to [`RenderConfig::supports`] to check whether the current
/// [`RenderConfig`] enables a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineCapability {
    /// 2D canvas rendering (requires [`GraphicsTier::Lite2D`]).
    Canvas2D,
    /// Real-time 3D lighting and shadows (requires at least [`GraphicsTier::Standard3D`]).
    Lighting3D,
    /// HDR pipeline (requires [`GraphicsTier::Advanced3D`] and `hdr = true`).
    Hdr,
    /// Post-processing effects (bloom, DOF, motion blur).
    PostProcessing,
    /// rapier3d physics (requires at least [`GraphicsTier::Standard3D`]).
    Physics3D,
    /// Advanced vehicle / joint physics (requires [`GraphicsTier::Advanced3D`]).
    VehiclePhysics,
    /// High-quality cascaded shadow maps.
    ShadowMaps,
}

// ---------------------------------------------------------------------------
// RenderConfig
// ---------------------------------------------------------------------------

/// Complete render and engine configuration for a Magnetite game.
///
/// Construct via [`RenderConfig::new`] (defaults) or [`RenderConfig::builder`].
///
/// ```rust
/// use magnetite_sdk::graphics::{GraphicsTier, RenderConfig};
///
/// let cfg = RenderConfig::new(GraphicsTier::Lite2D);
/// assert_eq!(cfg.tier, GraphicsTier::Lite2D);
/// assert_eq!(cfg.target_fps, 60);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderConfig {
    /// The graphics / engine tier.
    pub tier: GraphicsTier,

    /// Target frames per second.  Clamped to [`GraphicsTier::max_target_fps`].
    pub target_fps: u32,

    /// Shadow quality level (0 = off, 1 = low, 2 = medium, 3 = high, 4 = ultra).
    ///
    /// Values above 2 require at least [`GraphicsTier::Advanced3D`].
    pub shadow_quality: u8,

    /// Whether HDR rendering is enabled (requires [`GraphicsTier::Advanced3D`]).
    pub hdr: bool,

    /// Whether post-processing effects (bloom, DOF, motion blur) are enabled.
    /// Automatically enabled when `hdr = true` for `Advanced3D`.
    pub post_processing: bool,

    /// Number of physics simulation substeps per frame.
    ///
    /// Higher values improve simulation accuracy for fast-moving objects
    /// (e.g. race cars at 300 km/h).  Recommended: 1 (Lite2D), 4 (Standard3D),
    /// 8–16 (Advanced3D).
    pub physics_substeps: u32,

    /// Optional aspect-ratio hint (width / height) for the render viewport.
    /// `None` = use the window's native aspect ratio.
    pub aspect_ratio: Option<f32>,

    /// MSAA sample count (1 = off, 2, 4, 8). 1 is always valid; higher values
    /// require [`GraphicsTier::Standard3D`] or above.
    pub msaa_samples: u8,

    /// Maximum number of dynamic lights in the scene (point + spot).
    /// Ignored for [`GraphicsTier::Lite2D`].
    pub max_dynamic_lights: u32,
}

impl RenderConfig {
    /// Create a `RenderConfig` with sensible defaults for the given tier.
    pub fn new(tier: GraphicsTier) -> Self {
        let mut cfg = RenderConfig {
            tier,
            target_fps: tier.max_target_fps().min(60),
            shadow_quality: 0,
            hdr: false,
            post_processing: false,
            physics_substeps: tier.recommended_physics_substeps(),
            aspect_ratio: None,
            msaa_samples: 1,
            max_dynamic_lights: 0,
        };

        match tier {
            GraphicsTier::Lite2D => {
                // Keep defaults — no shadows, no 3D features.
            }
            GraphicsTier::Standard3D => {
                cfg.target_fps = 60;
                cfg.shadow_quality = 2;
                cfg.msaa_samples = 4;
                cfg.max_dynamic_lights = 32;
            }
            GraphicsTier::Advanced3D => {
                cfg.target_fps = 120;
                cfg.shadow_quality = 4;
                cfg.hdr = true;
                cfg.post_processing = true;
                cfg.msaa_samples = 4;
                cfg.max_dynamic_lights = 256;
            }
        }

        cfg
    }

    /// Start building a custom [`RenderConfig`].
    pub fn builder() -> RenderConfigBuilder {
        RenderConfigBuilder::default()
    }

    /// Returns `true` if this config enables the given [`EngineCapability`].
    ///
    /// ```rust
    /// use magnetite_sdk::graphics::{EngineCapability, GraphicsTier, RenderConfig};
    ///
    /// let cfg = RenderConfig::new(GraphicsTier::Advanced3D);
    /// assert!(cfg.supports(EngineCapability::Hdr));
    /// assert!(cfg.supports(EngineCapability::VehiclePhysics));
    /// assert!(!cfg.supports(EngineCapability::Canvas2D));
    /// ```
    pub fn supports(&self, cap: EngineCapability) -> bool {
        match cap {
            EngineCapability::Canvas2D => self.tier == GraphicsTier::Lite2D,
            EngineCapability::Lighting3D => self.tier.at_least(GraphicsTier::Standard3D),
            EngineCapability::Hdr => self.tier == GraphicsTier::Advanced3D && self.hdr,
            EngineCapability::PostProcessing => {
                self.tier == GraphicsTier::Advanced3D && self.post_processing
            }
            EngineCapability::Physics3D => self.tier.at_least(GraphicsTier::Standard3D),
            EngineCapability::VehiclePhysics => self.tier == GraphicsTier::Advanced3D,
            EngineCapability::ShadowMaps => self.shadow_quality > 0,
        }
    }

    /// Clamp target_fps to the tier's maximum.
    fn clamp_fps(tier: GraphicsTier, fps: u32) -> u32 {
        fps.min(tier.max_target_fps())
    }
}

// ---------------------------------------------------------------------------
// RenderConfigBuilder
// ---------------------------------------------------------------------------

/// Builder for [`RenderConfig`].
///
/// ```rust
/// use magnetite_sdk::graphics::{GraphicsTier, RenderConfig};
///
/// let cfg = RenderConfig::builder()
///     .tier(GraphicsTier::Advanced3D)
///     .target_fps(120)
///     .shadow_quality(4)
///     .hdr(true)
///     .post_processing(true)
///     .physics_substeps(8)
///     .build();
///
/// assert_eq!(cfg.tier, GraphicsTier::Advanced3D);
/// assert!(cfg.hdr);
/// assert_eq!(cfg.physics_substeps, 8);
/// ```
#[derive(Debug, Default)]
pub struct RenderConfigBuilder {
    tier: Option<GraphicsTier>,
    target_fps: Option<u32>,
    shadow_quality: Option<u8>,
    hdr: Option<bool>,
    post_processing: Option<bool>,
    physics_substeps: Option<u32>,
    aspect_ratio: Option<f32>,
    msaa_samples: Option<u8>,
    max_dynamic_lights: Option<u32>,
}

impl RenderConfigBuilder {
    /// Set the graphics tier.
    pub fn tier(mut self, tier: GraphicsTier) -> Self {
        self.tier = Some(tier);
        self
    }

    /// Set the target FPS.
    pub fn target_fps(mut self, fps: u32) -> Self {
        self.target_fps = Some(fps);
        self
    }

    /// Set the shadow quality (0–4).
    pub fn shadow_quality(mut self, quality: u8) -> Self {
        self.shadow_quality = Some(quality);
        self
    }

    /// Enable or disable HDR.
    pub fn hdr(mut self, hdr: bool) -> Self {
        self.hdr = Some(hdr);
        self
    }

    /// Enable or disable post-processing.
    pub fn post_processing(mut self, enabled: bool) -> Self {
        self.post_processing = Some(enabled);
        self
    }

    /// Set the physics substep count.
    pub fn physics_substeps(mut self, substeps: u32) -> Self {
        self.physics_substeps = Some(substeps);
        self
    }

    /// Set a fixed aspect ratio hint.
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    /// Set MSAA sample count.
    pub fn msaa_samples(mut self, samples: u8) -> Self {
        self.msaa_samples = Some(samples);
        self
    }

    /// Set the maximum number of dynamic lights.
    pub fn max_dynamic_lights(mut self, n: u32) -> Self {
        self.max_dynamic_lights = Some(n);
        self
    }

    /// Build the [`RenderConfig`], applying defaults for any unset fields.
    pub fn build(self) -> RenderConfig {
        let tier = self.tier.unwrap_or(GraphicsTier::Standard3D);
        let mut base = RenderConfig::new(tier);

        if let Some(fps) = self.target_fps {
            base.target_fps = RenderConfig::clamp_fps(tier, fps);
        }
        if let Some(q) = self.shadow_quality {
            base.shadow_quality = q;
        }
        if let Some(hdr) = self.hdr {
            base.hdr = hdr;
        }
        if let Some(pp) = self.post_processing {
            base.post_processing = pp;
        }
        if let Some(s) = self.physics_substeps {
            base.physics_substeps = s;
        }
        if let Some(ar) = self.aspect_ratio {
            base.aspect_ratio = Some(ar);
        }
        if let Some(ms) = self.msaa_samples {
            base.msaa_samples = ms;
        }
        if let Some(n) = self.max_dynamic_lights {
            base.max_dynamic_lights = n;
        }

        base
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GraphicsTier --

    #[test]
    fn tier_ordering() {
        assert!(GraphicsTier::Lite2D < GraphicsTier::Standard3D);
        assert!(GraphicsTier::Standard3D < GraphicsTier::Advanced3D);
    }

    #[test]
    fn tier_at_least() {
        assert!(GraphicsTier::Advanced3D.at_least(GraphicsTier::Lite2D));
        assert!(GraphicsTier::Advanced3D.at_least(GraphicsTier::Advanced3D));
        assert!(!GraphicsTier::Lite2D.at_least(GraphicsTier::Standard3D));
    }

    #[test]
    fn tier_labels_non_empty() {
        for tier in &[
            GraphicsTier::Lite2D,
            GraphicsTier::Standard3D,
            GraphicsTier::Advanced3D,
        ] {
            assert!(!tier.label().is_empty());
        }
    }

    #[test]
    fn tier_max_fps_increases_with_tier() {
        assert!(GraphicsTier::Lite2D.max_target_fps() <= GraphicsTier::Standard3D.max_target_fps());
        assert!(
            GraphicsTier::Standard3D.max_target_fps() <= GraphicsTier::Advanced3D.max_target_fps()
        );
    }

    #[test]
    fn tier_physics_substeps_increase_with_tier() {
        assert!(
            GraphicsTier::Lite2D.recommended_physics_substeps()
                <= GraphicsTier::Standard3D.recommended_physics_substeps()
        );
        assert!(
            GraphicsTier::Standard3D.recommended_physics_substeps()
                <= GraphicsTier::Advanced3D.recommended_physics_substeps()
        );
    }

    #[test]
    fn tier_serde_roundtrip() {
        for tier in &[
            GraphicsTier::Lite2D,
            GraphicsTier::Standard3D,
            GraphicsTier::Advanced3D,
        ] {
            let json = serde_json::to_string(tier).unwrap();
            let back: GraphicsTier = serde_json::from_str(&json).unwrap();
            assert_eq!(tier, &back);
        }
    }

    // -- RenderConfig::new --

    #[test]
    fn render_config_lite2d_defaults() {
        let cfg = RenderConfig::new(GraphicsTier::Lite2D);
        assert_eq!(cfg.tier, GraphicsTier::Lite2D);
        assert_eq!(cfg.shadow_quality, 0);
        assert!(!cfg.hdr);
        assert!(!cfg.post_processing);
        assert_eq!(cfg.physics_substeps, 1);
    }

    #[test]
    fn render_config_standard3d_defaults() {
        let cfg = RenderConfig::new(GraphicsTier::Standard3D);
        assert_eq!(cfg.tier, GraphicsTier::Standard3D);
        assert_eq!(cfg.shadow_quality, 2);
        assert!(!cfg.hdr);
        assert!(cfg.max_dynamic_lights > 0);
    }

    #[test]
    fn render_config_advanced3d_defaults() {
        let cfg = RenderConfig::new(GraphicsTier::Advanced3D);
        assert_eq!(cfg.tier, GraphicsTier::Advanced3D);
        assert!(cfg.hdr);
        assert!(cfg.post_processing);
        assert_eq!(cfg.shadow_quality, 4);
        assert_eq!(cfg.physics_substeps, 8);
    }

    // -- RenderConfig::supports --

    #[test]
    fn canvas2d_only_on_lite2d() {
        assert!(RenderConfig::new(GraphicsTier::Lite2D).supports(EngineCapability::Canvas2D));
        assert!(!RenderConfig::new(GraphicsTier::Standard3D).supports(EngineCapability::Canvas2D));
        assert!(!RenderConfig::new(GraphicsTier::Advanced3D).supports(EngineCapability::Canvas2D));
    }

    #[test]
    fn lighting3d_requires_at_least_standard() {
        assert!(!RenderConfig::new(GraphicsTier::Lite2D).supports(EngineCapability::Lighting3D));
        assert!(RenderConfig::new(GraphicsTier::Standard3D).supports(EngineCapability::Lighting3D));
        assert!(RenderConfig::new(GraphicsTier::Advanced3D).supports(EngineCapability::Lighting3D));
    }

    #[test]
    fn hdr_requires_advanced_and_enabled() {
        let advanced = RenderConfig::new(GraphicsTier::Advanced3D);
        assert!(advanced.supports(EngineCapability::Hdr));

        let mut no_hdr = advanced.clone();
        no_hdr.hdr = false;
        assert!(!no_hdr.supports(EngineCapability::Hdr));

        assert!(!RenderConfig::new(GraphicsTier::Standard3D).supports(EngineCapability::Hdr));
    }

    #[test]
    fn vehicle_physics_only_on_advanced() {
        assert!(!RenderConfig::new(GraphicsTier::Lite2D).supports(EngineCapability::VehiclePhysics));
        assert!(
            !RenderConfig::new(GraphicsTier::Standard3D).supports(EngineCapability::VehiclePhysics)
        );
        assert!(
            RenderConfig::new(GraphicsTier::Advanced3D).supports(EngineCapability::VehiclePhysics)
        );
    }

    #[test]
    fn shadow_maps_off_when_quality_zero() {
        let mut cfg = RenderConfig::new(GraphicsTier::Advanced3D);
        cfg.shadow_quality = 0;
        assert!(!cfg.supports(EngineCapability::ShadowMaps));
        cfg.shadow_quality = 1;
        assert!(cfg.supports(EngineCapability::ShadowMaps));
    }

    // -- RenderConfigBuilder --

    #[test]
    fn builder_basic() {
        let cfg = RenderConfig::builder()
            .tier(GraphicsTier::Standard3D)
            .target_fps(90)
            .shadow_quality(2)
            .build();
        assert_eq!(cfg.tier, GraphicsTier::Standard3D);
        assert_eq!(cfg.target_fps, 90);
        assert_eq!(cfg.shadow_quality, 2);
    }

    #[test]
    fn builder_fps_clamped_to_tier_max() {
        let cfg = RenderConfig::builder()
            .tier(GraphicsTier::Lite2D)
            .target_fps(9999)
            .build();
        assert_eq!(cfg.target_fps, GraphicsTier::Lite2D.max_target_fps());
    }

    #[test]
    fn builder_advanced_motorsport() {
        let cfg = RenderConfig::builder()
            .tier(GraphicsTier::Advanced3D)
            .target_fps(120)
            .shadow_quality(4)
            .hdr(true)
            .post_processing(true)
            .physics_substeps(8)
            .build();
        assert!(cfg.supports(EngineCapability::Hdr));
        assert!(cfg.supports(EngineCapability::VehiclePhysics));
        assert_eq!(cfg.physics_substeps, 8);
    }

    #[test]
    fn builder_default_tier_is_standard3d() {
        let cfg = RenderConfig::builder().build();
        assert_eq!(cfg.tier, GraphicsTier::Standard3D);
    }

    #[test]
    fn render_config_serde_roundtrip() {
        let cfg = RenderConfig::builder()
            .tier(GraphicsTier::Advanced3D)
            .hdr(true)
            .physics_substeps(16)
            .aspect_ratio(16.0 / 9.0)
            .build();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: RenderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn engine_capability_serde_roundtrip() {
        let caps = [
            EngineCapability::Canvas2D,
            EngineCapability::Lighting3D,
            EngineCapability::Hdr,
            EngineCapability::PostProcessing,
            EngineCapability::Physics3D,
            EngineCapability::VehiclePhysics,
            EngineCapability::ShadowMaps,
        ];
        for cap in &caps {
            let json = serde_json::to_string(cap).unwrap();
            let back: EngineCapability = serde_json::from_str(&json).unwrap();
            assert_eq!(cap, &back);
        }
    }
}

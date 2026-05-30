# Graphics Tiers

Every Magnetite game declares a **graphics tier** via `RenderConfig`. The platform uses
this declaration to provision the right runtime, CDN path, and server-side constraints.
Simple games stay lightweight; advanced games scale up.

---

## Tiers

| Tier | Target | Browser runtime | Server physics |
|------|--------|-----------------|----------------|
| `Lite2D` | 2D arcade, puzzle, platformer | Canvas 2D / WebGL | 2D physics (optional) |
| `Standard3D` | General 3D games, casual multiplayer | WebGL2 | rapier3d (basic) |
| `Advanced3D` | FPS, motorsport, AAA | WebGPU / Vulkan, HDR | rapier3d + substeps, SSAO |

---

## Declaring a tier

```rust
use magnetite_sdk::graphics::{GraphicsTier, RenderConfig};

// Simple arcade game — no 3D, minimal bundle.
let config = RenderConfig::new(GraphicsTier::Lite2D);

// General 3D game.
let config = RenderConfig::new(GraphicsTier::Standard3D);

// Full FPS / motorsport — HDR, high-fidelity physics.
let config = RenderConfig::builder()
    .tier(GraphicsTier::Advanced3D)
    .hdr(true)
    .physics_substeps(8)
    .shadows(true)
    .build();
```

---

## `RenderConfig` builder

```rust
pub struct RenderConfigBuilder {
    tier: GraphicsTier,
    hdr: bool,                // Default: false
    physics_substeps: u8,     // Default: 2
    shadows: bool,            // Default: false
    msaa_samples: u8,         // Default: 1
}
```

Call `RenderConfig::builder()` to get a `RenderConfigBuilder`, chain methods, then `.build()`.

---

## Engine capabilities

Use `EngineCapability` to query what the current platform supports at runtime:

```rust
use magnetite_sdk::graphics::{EngineCapability, RenderConfig};

let config = RenderConfig::new(GraphicsTier::Advanced3D);

if config.capability() >= EngineCapability::WebGpu {
    // Enable ray-tracing effects.
}
```

| `EngineCapability` | Description |
|--------------------|-------------|
| `Canvas2d` | Basic 2D rendering |
| `WebGl` | WebGL 1 — broad compatibility |
| `WebGl2` | WebGL 2 — instancing, MRTs |
| `WebGpu` | WebGPU — compute shaders, HDR |
| `NativeVulkan` | Desktop native — full feature set |

---

## Platform behaviour per tier

| Tier | WASM bundle limit | CDN priority | Server allocation |
|------|-------------------|--------------|-------------------|
| `Lite2D` | 2 MB | Aggressive edge cache | Shared micro instance |
| `Standard3D` | 20 MB | Standard CDN | Shared standard instance |
| `Advanced3D` | 100 MB | Standard CDN | Dedicated or dedicated-class shared |

These are documented targets. The current foundation provisions a single instance class;
per-tier allocation is the production roadmap.

---

## Bevy feature flags

The game templates map tiers to Bevy feature sets via Cargo feature flags:

```toml
[features]
# Lite2D: 2D Bevy, no 3D renderer.
default = []

# Standard3D: Bevy with default renderer (WebGL2 / Vulkan).
native = ["dep:bevy", "bevy/default"]

# Advanced3D: full render stack + rapier3d physics substeps.
wasm = ["dep:bevy", "dep:bevy_rapier3d"]
```

See `game-template-fps/Cargo.toml` and `game-template-motorsport/Cargo.toml` for
concrete examples.

---

## See also

- [SDK Reference](./sdk.md) — `graphics` module
- [FPS Starter Template](./fps-starter.md) — Advanced3D example
- [Motorsport Starter Template](./motorsport-starter.md) — Advanced3D + physics substeps
- [Controllers & Gamepad Input](./controllers.md)

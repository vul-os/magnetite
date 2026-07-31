<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# Roadmap — reference games

Magnetite ships four **starter game crates** under `game-templates/` (the
directory name *is* the template catalog id — a developer scaffolds a new game
from one with `magnetite scaffold <name> --template <id>`, see the
quick start (repo README.md)). Those are the scaffolds. The *reference
games* built on them — polished, playable proof that each tier of the engine
works end-to-end through the wasm sandbox and replay-verification path — are
what this roadmap tracks. The templates are structure-tested in
`backend/tests/gds_tests.rs`; the games on top of them are the work.

## Built

- **Arena shooter** — `game-templates/authoritative`. The `AuthoritativeGame`
  reference, and the one exercised for real today: it compiles to
  `wasm32-unknown-unknown` against the `mag_*` sandbox ABI and runs client-side
  as a `Topology::SingleRoom` match — the bottom rung of the topology ladder,
  no server. [wibbly](https://github.com/vul-os/wibbly) drives exactly this
  wasm build in the browser.

## Games to make

- **FPS** — `game-templates/fps` (Bevy + rapier3d, hitscan, gamepad). The first
  **standard-3D** reference: a small deterministic first-person arena proving
  hitscan and 3D physics survive the sandbox + deterministic-replay path and
  cascade across the topology tiers the way the arena shooter does.
- **Motorsport** — `game-templates/motorsport` (Bevy + rapier3d, lap → points).
  The first **advanced-3D** reference: vehicle dynamics and lap scoring — the
  heaviest simulation tier — proving the determinism guarantees hold under
  rapier3d vehicle physics.

## On-ramp

- **2D arcade** — `game-templates/arcade` (Bevy + `GameLogic`, WASM-ready). A
  lightweight 2D reference game here is a good first-contribution on-ramp; not
  yet built.

---

For the platform/engine backlog (the seams, the decentralization waves) rather
than games, see [Decentralization](#decentralization) §5, and
`ALIGNMENT.md` in the repo for the kotva/patala binding direction, the
reachability rungs and the phased platform roadmap.

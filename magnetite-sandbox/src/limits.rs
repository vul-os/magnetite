//! Resource-limit configuration for the Wasmtime sandbox.

use wasmtime::ResourceLimiter;

/// Configuration for per-step and per-match resource limits.
///
/// All fields have conservative defaults suitable for a moderately-complex
/// authoritative game step.  Tune them based on your game's complexity.
///
/// # Example
/// ```rust
/// use magnetite_sandbox::LimitsConfig;
///
/// // Default limits.
/// let limits = LimitsConfig::default();
/// assert!(limits.fuel_per_step > 0);
/// assert!(limits.max_memory_bytes > 0);
///
/// // Custom limits for a simpler game.
/// let custom = LimitsConfig {
///     fuel_per_step: 1_000_000,
///     max_memory_bytes: 16 * 1024 * 1024,
///     max_epochs_per_step: 1,
///     epoch_tick_ms: 10,
/// };
/// assert_eq!(custom.max_memory_bytes, 16 * 1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct LimitsConfig {
    /// Wasmtime fuel units consumed per `mag_step` call.
    ///
    /// Each Wasm instruction consumes some amount of fuel (roughly 1 unit per
    /// simple instruction).  A typical authoritative game step at 60 Hz should
    /// complete in well under 10 million units.
    pub fuel_per_step: u64,

    /// Maximum guest linear memory in bytes.
    ///
    /// The `wasmtime::ResourceLimiter` implementation enforces this cap on
    /// both initial allocation and `memory.grow` calls.  The default (64 MiB)
    /// is generous for most games; cap lower for stricter sandboxing.
    pub max_memory_bytes: usize,

    /// Number of engine epochs before a running step is interrupted.
    ///
    /// The epoch counter is incremented by a background thread every
    /// [`epoch_tick_ms`](LimitsConfig::epoch_tick_ms) milliseconds.  The store
    /// is configured with `set_epoch_deadline(max_epochs_per_step)`, so a step
    /// that takes longer than `epoch_tick_ms × max_epochs_per_step` is killed.
    pub max_epochs_per_step: u64,

    /// How often the background thread increments the epoch, in milliseconds.
    pub epoch_tick_ms: u64,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            // ~10M instructions worth of headroom for a 60 Hz game step.
            fuel_per_step: 10_000_000,
            // 64 MiB — comfortable upper bound for authoritative game state.
            max_memory_bytes: 64 * 1024 * 1024,
            // Interrupt after 2 epochs (2 × 5 ms = 10 ms wall-clock budget).
            max_epochs_per_step: 2,
            epoch_tick_ms: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// StoreLimits — ResourceLimiter implementation
// ---------------------------------------------------------------------------

/// Wasmtime [`ResourceLimiter`] enforcing a per-instance memory cap.
///
/// Plugged into the store via `store.limiter(|state| &mut state.limits)`.
pub(crate) struct StoreLimits {
    pub max_memory_bytes: usize,
}

impl ResourceLimiter for StoreLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        let _ = current;
        // Deny growth that would exceed the cap.
        Ok(desired <= self.max_memory_bytes)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        let _ = current;
        // Allow table growth up to 64K elements — tables hold function pointers
        // and are generally small.
        Ok(desired <= 65_536)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_positive() {
        let l = LimitsConfig::default();
        assert!(l.fuel_per_step > 0, "fuel_per_step must be > 0");
        assert!(l.max_memory_bytes > 0, "max_memory_bytes must be > 0");
        assert!(l.max_epochs_per_step > 0, "max_epochs_per_step must be > 0");
        assert!(l.epoch_tick_ms > 0, "epoch_tick_ms must be > 0");
    }

    #[test]
    fn custom_limits_roundtrip() {
        let l = LimitsConfig {
            fuel_per_step: 999,
            max_memory_bytes: 1024,
            max_epochs_per_step: 3,
            epoch_tick_ms: 2,
        };
        assert_eq!(l.fuel_per_step, 999);
        assert_eq!(l.max_memory_bytes, 1024);
        assert_eq!(l.max_epochs_per_step, 3);
        assert_eq!(l.epoch_tick_ms, 2);
    }

    #[test]
    fn store_limits_allows_under_cap() {
        let mut lim = StoreLimits {
            max_memory_bytes: 1024 * 1024,
        };
        // 512 KiB desired — under the 1 MiB cap.
        let result = lim.memory_growing(0, 512 * 1024, None).unwrap();
        assert!(result, "allocation under cap must be allowed");
    }

    #[test]
    fn store_limits_denies_over_cap() {
        let mut lim = StoreLimits {
            max_memory_bytes: 512 * 1024,
        };
        // 1 MiB desired — over the 512 KiB cap.
        let result = lim.memory_growing(0, 1024 * 1024, None).unwrap();
        assert!(!result, "allocation over cap must be denied");
    }

    #[test]
    fn store_limits_allows_table_growth() {
        let mut lim = StoreLimits {
            max_memory_bytes: 0,
        };
        let result = lim.table_growing(0usize, 100usize, None).unwrap();
        assert!(result);
    }

    #[test]
    fn store_limits_denies_huge_table() {
        let mut lim = StoreLimits {
            max_memory_bytes: 0,
        };
        let result = lim.table_growing(0usize, 100_000usize, None).unwrap();
        assert!(!result, "enormous table must be denied");
    }
}

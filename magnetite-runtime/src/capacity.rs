//! Hardware capacity measurement (DECENTRALIZATION.md §4).
//!
//! A node **measures its own hardware** and advertises a
//! [`magnetite_seams::discovery::Capacity`]. The player/shard cap it can host is
//! then **emergent from that measurement — never a hardcoded constant.** More
//! cores ⇒ more shards ⇒ more players, with no code change.
//!
//! Measurement uses only the standard library plus a couple of best-effort OS
//! probes (`/proc/meminfo` on Linux, `sysctl` on macOS). If a probe is
//! unavailable the field degrades to a conservative default — the node still
//! advertises *something*, it just under-claims. No external crate is required.

use magnetite_seams::discovery::Capacity;
use magnetite_sdk::scaling::{player_capacity, shards_for_capacity, DEFAULT_PLAYERS_PER_SHARD};

/// Conservative RAM fallback (MB) when the OS probe fails.
const RAM_FALLBACK_MB: u64 = 4096;
/// Conservative bandwidth assumption (Mbps). Real measurement needs a probe we
/// deliberately don't ship offline; operators can override via env.
const DEFAULT_BANDWIDTH_MBPS: u32 = 1000;

/// Measure this box's capacity and derive its **emergent** shard/player budget.
///
/// The returned [`Capacity`] carries:
/// - `cpu_cores` — from [`std::thread::available_parallelism`].
/// - `ram_mb` — best-effort OS probe, else [`RAM_FALLBACK_MB`].
/// - `max_shards` — [`shards_for_capacity`], i.e. emergent from cores *and* RAM.
/// - `free_slots` — the full emergent player capacity at boot (no players yet).
///
/// Environment overrides (useful in containers / CI):
/// - `MAGNETITE_CORES`, `MAGNETITE_RAM_MB`, `MAGNETITE_BANDWIDTH_MBPS`.
pub fn measure_capacity() -> Capacity {
    let cpu_cores = env_u32("MAGNETITE_CORES").unwrap_or_else(detect_cores);
    let ram_mb = env_u64("MAGNETITE_RAM_MB").unwrap_or_else(detect_ram_mb);
    let bandwidth_mbps = env_u32("MAGNETITE_BANDWIDTH_MBPS").unwrap_or(DEFAULT_BANDWIDTH_MBPS);

    // Seed a bare capacity, then let the SDK derive the emergent budget from it.
    let mut cap = Capacity {
        cpu_cores,
        ram_mb,
        bandwidth_mbps,
        free_slots: 0,
        max_shards: 0,
    };
    cap.max_shards = shards_for_capacity(&cap);
    cap.free_slots =
        u32::try_from(player_capacity(&cap, DEFAULT_PLAYERS_PER_SHARD)).unwrap_or(u32::MAX);
    cap
}

/// Recompute `free_slots` from the currently occupied seats, keeping the
/// advertised capacity honest as players join/leave. Emergent total minus used.
pub fn with_occupancy(mut cap: Capacity, occupied_slots: u32) -> Capacity {
    let total = player_capacity(&cap, DEFAULT_PLAYERS_PER_SHARD);
    let total = u32::try_from(total).unwrap_or(u32::MAX);
    cap.free_slots = total.saturating_sub(occupied_slots);
    cap
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

fn detect_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
        .max(1)
}

/// Best-effort total RAM in MB. Zero external deps.
fn detect_ram_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Some(mb) = linux_ram_mb() {
            return mb;
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(mb) = macos_ram_mb() {
            return mb;
        }
    }
    RAM_FALLBACK_MB
}

#[cfg(target_os = "linux")]
fn linux_ram_mb() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            // Format: "MemTotal:       16318612 kB"
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_ram_mb() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    let bytes: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(bytes / (1024 * 1024))
}

fn env_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok()?.parse().ok()
}
fn env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok()?.parse().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_capacity_is_emergent_not_constant() {
        let cap = measure_capacity();
        // Real hardware always has at least one core and some RAM.
        assert!(cap.cpu_cores >= 1);
        assert!(cap.ram_mb >= 1);
        // Shard/player budget is DERIVED, never zero.
        assert!(cap.max_shards >= 1);
        assert!(cap.free_slots >= DEFAULT_PLAYERS_PER_SHARD);
        // The advertised budget matches the SDK's emergent derivation.
        assert_eq!(cap.max_shards, shards_for_capacity(&cap));
    }

    #[test]
    fn env_overrides_take_effect() {
        std::env::set_var("MAGNETITE_CORES", "16");
        std::env::set_var("MAGNETITE_RAM_MB", "65536");
        let cap = measure_capacity();
        std::env::remove_var("MAGNETITE_CORES");
        std::env::remove_var("MAGNETITE_RAM_MB");
        assert_eq!(cap.cpu_cores, 16);
        assert_eq!(cap.ram_mb, 65536);
        assert_eq!(cap.max_shards, 16, "16 cores + ample RAM ⇒ 16 shards");
    }

    #[test]
    fn occupancy_reduces_free_slots() {
        let cap = measure_capacity();
        let total = cap.free_slots;
        let busy = super::with_occupancy(cap, 10);
        assert_eq!(busy.free_slots, total.saturating_sub(10));
    }
}

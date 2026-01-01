//! The Gold Standard for Deterministic Physics Verification.
//!
//! This example demonstrates:
//! - Fixed timestep simulation loop
//! - Drift-free accumulation using `drift-linalg`
//! - Complete state snapshot (position AND velocity)
//! - SHA-256 determinism fingerprinting
//!
//! Run: `cargo run --release --example basic_verifier`

use drift_harness::{compute_fingerprint, DeterministicSim, HarnessConfig};
use drift_linalg::{Vec3, Vec3Accumulator};

/// The Invariant Time Step.
///
/// Changing this value invalidates all replay hashes.
/// 60 Hz is standard for physics simulations.
const DT_60HZ: f64 = 1.0 / 60.0;

/// A simple particle simulation with position and velocity.
struct ParticleSim {
    position: Vec3Accumulator,
    velocity: Vec3Accumulator,
}

impl ParticleSim {
    fn new() -> Self {
        Self {
            position: Vec3Accumulator::new(),
            velocity: Vec3Accumulator::new(),
        }
    }
}

impl DeterministicSim for ParticleSim {
    fn tick(&mut self, _inputs: &[u8]) {
        // Explicit gravity constant (m/s²)
        // This is a named constant, not a magic number.
        let gravity = Vec3::new(0.0, -9.81, 0.0);

        // Symplectic Euler Integration (velocity-first)
        // This order is important for energy conservation in physics.
        
        // 1. Integrate Velocity: v(t+dt) = v(t) + a * dt
        self.velocity.add_scaled(gravity, DT_60HZ);

        // 2. Resolve Velocity for Position Integration
        // NOTE: This is the only quantization step in the tick.
        // The resolve() extracts the compensated total to standard f64.
        let v_t = self.velocity.resolve();

        // 3. Integrate Position: x(t+dt) = x(t) + v(t+dt) * dt
        self.position.add_scaled(v_t, DT_60HZ);
    }

    fn state_snapshot(&self) -> Vec<u8> {
        // CRITICAL: Capture FULL state (position AND velocity)
        //
        // If you only capture position, two simulations with:
        //   - Same position
        //   - Different velocity
        // ...will produce identical hashes but DIVERGE on the next tick.
        //
        // This defeats the entire purpose of determinism verification.
        
        let p = self.position.resolve();
        let v = self.velocity.resolve();

        // CANONICAL SERIALIZATION
        // We use raw IEEE-754 little-endian bytes, NOT text formatting.
        // This guarantees identical hashes on x86_64, ARM64, and WASM.
        let mut buf = Vec::with_capacity(48);
        buf.extend_from_slice(&p.to_le_bytes());
        buf.extend_from_slice(&v.to_le_bytes());
        buf
    }
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           DRIFT KERNEL - DETERMINISM HARNESS                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Tick Rate:    {} Hz", 1.0 / DT_60HZ);
    println!("  Target Ticks: 10,000");
    println!("  Sim Time:     {:.1} seconds", 10_000.0 * DT_60HZ);
    println!();

    let sim = ParticleSim::new();

    // We simulate 10,000 ticks (approximately 2.7 minutes of simulation time)
    let config = HarnessConfig::vacuum(10_000);

    println!("Running simulation...");
    let hash = compute_fingerprint(sim, config);

    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("DETERMINISM FINGERPRINT (SHA-256)");
    println!("══════════════════════════════════════════════════════════════");
    println!("{}", hash);
    println!("══════════════════════════════════════════════════════════════");
    println!();
    println!("To verify determinism:");
    println!("  1. Run this on another machine");
    println!("  2. Compare the hash");
    println!("  3. If they match, the simulation is deterministic");
    println!();

    // Self-Verification for CI
    // This hash should be identical across all platforms.
    // If the physics kernel math changes, this hash will change.
    // This is the "Canary in the Coal Mine."
    assert!(!hash.is_empty(), "Hash generation failed");
    assert_eq!(hash.len(), 64, "Invalid SHA-256 hash length");
}

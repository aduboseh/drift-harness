//! Drift-Harness — Determinism Verification for Physics Simulations
//!
//! This crate provides a minimal framework for verifying that simulations
//! built on `drift-kernel` and `drift-linalg` produce deterministic results.
//!
//! # Usage
//!
//! 1. Implement [`DeterministicSim`] for your simulation
//! 2. Call [`compute_fingerprint`] to generate a hash of the final state
//! 3. Compare fingerprints across runs/platforms to verify determinism
//!
//! # Example
//!
//! ```rust,ignore
//! use drift_harness::{DeterministicSim, HarnessConfig, compute_fingerprint};
//!
//! struct MySim { /* ... */ }
//!
//! impl DeterministicSim for MySim {
//!     fn tick(&mut self, inputs: &[u8]) { /* ... */ }
//!     fn state_snapshot(&self) -> Vec<u8> { /* ... */ }
//! }
//!
//! let hash = compute_fingerprint(MySim::new(), HarnessConfig::default());
//! println!("Determinism fingerprint: {}", hash);
//! ```

use sha2::{Digest, Sha256};

mod trace;
pub use trace::{compute_trace_fingerprint, verify_trace_against, TraceError};

/// Trait for simulations
///
/// Implementors must provide:
/// - A `tick` function that advances the simulation by one fixed timestep
/// - A `state_snapshot` function that serializes ALL mutable state to bytes
///
/// # Contract
///
/// For determinism verification to be valid:
/// - `tick` must be pure: same inputs → same state transition
/// - `state_snapshot` must capture ALL state that affects future ticks
/// - Snapshot serialization must be platform-consistent (use `to_le_bytes`, not text)
pub trait DeterministicSim {
    /// Advance the simulation by one tick.
    ///
    /// # Arguments
    ///
    /// * `inputs` - External inputs for this tick (empty for vacuum tests)
    ///
    /// # Determinism Requirement
    ///
    /// Given the same prior state and inputs, this function must produce
    /// identical results across all runs and platforms.
    fn tick(&mut self, inputs: &[u8]);

    /// Serialize all mutable state to bytes.
    ///
    /// # Requirements
    ///
    /// - Must include ALL state that affects future ticks (position, velocity, etc.)
    /// - Must use little-endian byte serialization (`to_le_bytes`)
    /// - Must NOT use text formatting (Debug, Display, format!)
    ///
    /// # Why This Matters
    ///
    /// If you omit state (e.g., velocity), two simulations with identical
    /// positions but different velocities will hash identically—then diverge
    /// on the next tick. This defeats the purpose of determinism verification.
    fn state_snapshot(&self) -> Vec<u8>;
}

/// Configuration for determinism
#[derive(Debug, Clone)]
pub struct HarnessConfig {
    /// Number of ticks to simulate.
    pub target_ticks: u64,
    /// Input sequence (one entry per tick, or empty for vacuum test).
    pub inputs: Vec<Vec<u8>>,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            target_ticks: 1000,
            inputs: Vec::new(),
        }
    }
}

impl HarnessConfig {
    /// Create a vacuum test config (no external inputs).
    pub fn vacuum(ticks: u64) -> Self {
        Self {
            target_ticks: ticks,
            inputs: Vec::new(),
        }
    }

    /// Create a config with a fixed input sequence.
    pub fn with_inputs(ticks: u64, inputs: Vec<Vec<u8>>) -> Self {
        Self {
            target_ticks: ticks,
            inputs,
        }
    }
}

/// Execute a simulation
///
/// This function:
/// 1. Runs the simulation for `config.target_ticks` iterations
/// 2. Captures a state snapshot after the final tick
/// 3. Returns the SHA-256 hash of that snapshot as a hex string
///
/// # Note
///
/// This function **computes** a fingerprint; it does not **verify** anything.
/// To verify determinism, run this function twice (or on two platforms) and
/// compare the resulting hashes.
///
/// # Example
///
/// ```rust,ignore
/// let hash1 = compute_fingerprint(sim.clone(), config.clone());
/// let hash2 = compute_fingerprint(sim.clone(), config.clone());
/// assert_eq!(hash1, hash2, "Simulation is non-deterministic!");
/// ```
pub fn compute_fingerprint<S: DeterministicSim>(mut sim: S, config: HarnessConfig) -> String {
    let empty_input: Vec<u8> = Vec::new();

    for tick in 0..config.target_ticks {
        let input = config.inputs.get(tick as usize).unwrap_or(&empty_input);
        sim.tick(input);
    }

    let snapshot = sim.state_snapshot();

    let mut hasher = Sha256::new();
    hasher.update(&snapshot);
    let result = hasher.finalize();

    // Convert to hex string
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Verify that a simulation produces an expected fingerprint.
///
/// Returns `Ok(())` if the computed fingerprint matches the expected one,
/// or `Err` with details if they differ.
pub fn verify_against<S: DeterministicSim>(
    sim: S,
    config: HarnessConfig,
    expected_hash: &str,
) -> Result<(), DeterminismError> {
    let actual_hash = compute_fingerprint(sim, config);

    if actual_hash == expected_hash {
        Ok(())
    } else {
        Err(DeterminismError::HashMismatch {
            expected: expected_hash.to_string(),
            actual: actual_hash,
        })
    }
}

/// Errors that can occur
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeterminismError {
    /// The computed hash did not match the expected hash.
    HashMismatch { expected: String, actual: String },
}

impl std::fmt::Display for DeterminismError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeterminismError::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "Determinism verification failed!\n  Expected: {}\n  Actual:   {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for DeterminismError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct CounterSim {
        value: u64,
    }

    impl DeterministicSim for CounterSim {
        fn tick(&mut self, _inputs: &[u8]) {
            self.value = self.value.wrapping_add(1);
        }

        fn state_snapshot(&self) -> Vec<u8> {
            self.value.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn test_deterministic_counter() {
        let sim1 = CounterSim { value: 0 };
        let sim2 = CounterSim { value: 0 };
        let config = HarnessConfig::vacuum(100);

        let hash1 = compute_fingerprint(sim1, config.clone());
        let hash2 = compute_fingerprint(sim2, config);

        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());
    }

    #[test]
    fn test_verify_against_success() {
        let sim = CounterSim { value: 0 };
        let config = HarnessConfig::vacuum(100);

        // Pre-compute the expected hash
        let expected = compute_fingerprint(CounterSim { value: 0 }, config.clone());

        // Verify against it
        let result = verify_against(sim, config, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_against_failure() {
        let sim = CounterSim { value: 0 };
        let config = HarnessConfig::vacuum(100);

        let result = verify_against(sim, config, "wrong_hash");
        assert!(result.is_err());
    }
}

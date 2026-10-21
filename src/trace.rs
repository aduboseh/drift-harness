//! Path-sensitive deterministic replay fingerprints.
//!
//! Unlike a final-state-only fingerprint, this API hashes the initial state,
//! every consumed input, and every post-tick state, with explicit framing.
//! Determinism remains conditional on complete canonical state serialization
//! and a controlled execution environment.

use crate::{DeterministicSim, HarnessConfig};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    TickCountOverflow,
    InputCountMismatch { ticks: u64, inputs: usize },
    HashMismatch { expected: String, actual: String },
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TickCountOverflow => write!(f, "target tick count does not fit usize"),
            Self::InputCountMismatch { ticks, inputs } => write!(
                f,
                "trace needs exactly {ticks} input frames (or zero for vacuum); got {inputs}"
            ),
            Self::HashMismatch { expected, actual } => write!(
                f,
                "trace fingerprint mismatch: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl std::error::Error for TraceError {}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Fingerprint the entire ordered path, including initial state, inputs, and
/// all post-tick snapshots. This does not assert state-snapshot completeness.
///
/// An empty `inputs` vector explicitly requests vacuum (empty input each tick).
/// Otherwise the caller must provide exactly one input per tick.
pub fn compute_trace_fingerprint<S: DeterministicSim>(
    mut sim: S,
    config: HarnessConfig,
) -> Result<String, TraceError> {
    let target: usize = config
        .target_ticks
        .try_into()
        .map_err(|_| TraceError::TickCountOverflow)?;
    if !config.inputs.is_empty() && config.inputs.len() != target {
        return Err(TraceError::InputCountMismatch {
            ticks: config.target_ticks,
            inputs: config.inputs.len(),
        });
    }

    let mut hasher = Sha256::new();
    hasher.update(b"drift-harness/trace/v1");
    hasher.update(config.target_ticks.to_le_bytes());
    frame(&mut hasher, &sim.state_snapshot());

    for tick in 0..config.target_ticks {
        let input: &[u8] = if config.inputs.is_empty() {
            &[]
        } else {
            &config.inputs[tick as usize]
        };
        hasher.update(tick.to_le_bytes());
        frame(&mut hasher, input);
        sim.tick(input);
        frame(&mut hasher, &sim.state_snapshot());
    }

    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Verify an ordered replay path against a previously attested fingerprint.
pub fn verify_trace_against<S: DeterministicSim>(
    sim: S,
    config: HarnessConfig,
    expected: &str,
) -> Result<(), TraceError> {
    let actual = compute_trace_fingerprint(sim, config)?;
    if actual == expected {
        Ok(())
    } else {
        Err(TraceError::HashMismatch {
            expected: expected.to_owned(),
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute_fingerprint;

    #[derive(Clone)]
    struct PathSim {
        value: i32,
    }

    impl DeterministicSim for PathSim {
        fn tick(&mut self, input: &[u8]) {
            match input {
                b"+" => self.value += 1,
                b"-" => self.value -= 1,
                _ => {}
            }
        }

        fn state_snapshot(&self) -> Vec<u8> {
            self.value.to_le_bytes().to_vec()
        }
    }

    #[test]
    fn different_paths_same_final_state_have_different_trace_hashes() {
        let a = HarnessConfig::with_inputs(2, vec![b"+".to_vec(), b"-".to_vec()]);
        let b = HarnessConfig::with_inputs(2, vec![b"-".to_vec(), b"+".to_vec()]);
        assert_eq!(
            compute_fingerprint(PathSim { value: 0 }, a.clone()),
            compute_fingerprint(PathSim { value: 0 }, b.clone())
        );
        assert_ne!(
            compute_trace_fingerprint(PathSim { value: 0 }, a).unwrap(),
            compute_trace_fingerprint(PathSim { value: 0 }, b).unwrap()
        );
    }

    #[test]
    fn rejects_partial_input_sequences_in_strict_trace_mode() {
        let config = HarnessConfig::with_inputs(2, vec![b"+".to_vec()]);
        assert_eq!(
            compute_trace_fingerprint(PathSim { value: 0 }, config),
            Err(TraceError::InputCountMismatch {
                ticks: 2,
                inputs: 1
            })
        );
    }

    #[test]
    fn identical_vacuum_runs_verify_and_mutated_expectations_fail() {
        let config = HarnessConfig::vacuum(4);
        let expected = compute_trace_fingerprint(PathSim { value: 7 }, config.clone()).unwrap();
        verify_trace_against(PathSim { value: 7 }, config.clone(), &expected).unwrap();
        assert!(matches!(
            verify_trace_against(PathSim { value: 8 }, config, &expected),
            Err(TraceError::HashMismatch { .. })
        ));
    }
}

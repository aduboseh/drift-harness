# drift-harness

**Determinism verification harness for physics simulations.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Verify that simulations built on [drift-kernel](https://github.com/aduboseh/drift-kernel) produce bit-exact results across runs and platforms.

## What This Does

1. Runs your simulation for N ticks
2. Captures a state snapshot
3. Computes a SHA-256 fingerprint
4. Compare fingerprints to verify determinism

## Usage

```rust
use drift_harness::{DeterministicSim, HarnessConfig, compute_fingerprint};

struct MySim { /* ... */ }

impl DeterministicSim for MySim {
    fn tick(&mut self, inputs: &[u8]) {
        // Advance simulation by one fixed timestep
    }
    
    fn state_snapshot(&self) -> Vec<u8> {
        // Serialize ALL mutable state to bytes
        // Use to_le_bytes(), NOT text formatting
    }
}

let hash = compute_fingerprint(MySim::new(), HarnessConfig::vacuum(10_000));
println!("Determinism fingerprint: {}", hash);
```

## Verification

```rust
use drift_harness::verify_against;

let result = verify_against(sim, config, "expected_hash_here");
assert!(result.is_ok(), "Simulation is non-deterministic!");
```

## Critical Contract

For determinism verification to be valid:
- `tick()` must be pure: same inputs → same state transition
- `state_snapshot()` must capture **ALL** state that affects future ticks
- Use `to_le_bytes()` for serialization, never text formatting

If you omit state (e.g., velocity), two simulations with identical positions but different velocities will hash identically — then diverge on the next tick.

## Example

See `examples/basic_verifier.rs` for a complete particle simulation with determinism verification.

```bash
cargo run --release --example basic_verifier
```

## License

Apache-2.0

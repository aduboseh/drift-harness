# drift-harness

**Determinism verification harness for physics simulations.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Compare reproducible simulation executions under a controlled runtime, canonical serialization, and complete state capture. Bit-exact cross-platform behavior is **not** guaranteed by this library.

## What This Does

1. Runs your simulation for N ticks
2. Captures a state snapshot
3. Computes a SHA-256 fingerprint
4. Compare fingerprints to test replay equivalence within the declared environment; a matching digest is not proof of state completeness

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

## Ordered-path verification

The legacy `compute_fingerprint` API hashes only the final state. Different paths can converge to the same final state and produce identical hashes. The additive strict trace API includes initial state, every input frame and every post-tick snapshot:

```rust
use drift_harness::{compute_trace_fingerprint, HarnessConfig};
let trace = compute_trace_fingerprint(sim, HarnessConfig::vacuum(10_000))?;
println!("{trace}");
```

Non-vacuum trace verification rejects partial input arrays instead of silently treating missing frames as empty. The legacy final-state fingerprint retains its original behavior for compatibility.

The trace scheme uses versioned, length-prefixed SHA-256 framing and does **not** prove that the application's `state_snapshot()` includes all future-affecting variables. Canonical serialization and toolchain/architecture scope remain integrator responsibilities.

## Verification

```rust
use drift_harness::verify_against;

let result = verify_against(sim, config, "expected_hash_here");
assert!(result.is_ok(), "Simulation is non-deterministic!");
```

## Critical Contract

For determinism verification to be valid:
- `tick()` must be deterministic given canonical prior state, complete input and a controlled runtime
- `state_snapshot()` must capture **ALL** state that affects future ticks
- Use defined canonical binary encoding, including float treatment and schema version; raw `to_le_bytes()` alone is not sufficient to normalize all floating-point representations

If you omit state (e.g., velocity), two simulations with identical positions but different velocities may hash identically and later diverge. A path-sensitive trace detects many, but not all, such errors.

## Example

See `examples/basic_verifier.rs` for a complete particle simulation with determinism verification.

```bash
cargo run --release --example basic_verifier
```

## License

Apache-2.0

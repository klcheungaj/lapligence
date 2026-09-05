# sim/rt

Embedded C11 simulator and waveform runtime plus the libaco source snapshot used
when building generated models. These sources are compiled with each generated
model, not linked into Rust binaries.

Keep vector semantics aligned with `core::elab`, check allocation/size/time
boundaries, and cover scheduler behavior through the standalone C self-tests and
Rust process-level integration tests.

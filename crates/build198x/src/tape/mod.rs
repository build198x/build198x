//! Tape mastering: composing a loader, a loading screen and a program onto
//! one tape, per machine. Tape formats share an extension but nothing else (a
//! Spectrum `.tap` is a block stream, a C64 `.tap` a pulse recording), so
//! each machine has its own module, named by manufacturer and system as the
//! format crates are (`decisions/module-and-crate-naming.md`).

pub mod sinclair_zx_spectrum;

/// The machine ids `build198x tape --machine` accepts, as `mediaspec198x`
/// and `build198x basic` spell them.
pub const MACHINES: &[&str] = &["sinclair-zx-spectrum"];

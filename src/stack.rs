//! How deep a design may nest, and the stack that holds it.
//!
//! The frontend, the database import, lint, lowering, validation, the
//! optimizer and emission walk trees that are as deep as the source nests. One
//! limit bounds that depth and one reservation holds a tree of that depth in
//! every stage and every build profile; both are stated here only.
//!
//! - [`MAX_NESTING_DEPTH`] bounds what one source nests. The parser refuses
//!   the rest at the token that exceeds it.
//! - A walk that also follows what the source only names (the body a call
//!   enters, the class a constructor builds) is as deep as a chain of such
//!   names. Those walks keep their steps in a list where they can and ask
//!   [`exhausted`] where they cannot.
//!
//! # Margin
//!
//! In an unoptimized build one level costs at most 45 KiB of stack in the
//! costliest stage (a fixed-array index in an index, lowering), so a source at
//! the limit uses at most 180 MiB of [`PIPELINE_STACK_BYTES`]. An optimized
//! build uses less than half of that.

use std::cell::Cell;
use std::fmt;

/// Nesting depth at which a source is refused: the recursive constructs open
/// at a token, plus the operators, selections and dimensions that chain onto
/// them. Generated code nests a few dozen levels.
pub const MAX_NESTING_DEPTH: u32 = 4096;

/// Stack reserved for a pipeline thread. Address space: the system commits a
/// page when a stage first reaches it, so an ordinary design uses a few
/// hundred KiB of it.
pub const PIPELINE_STACK_BYTES: usize = 1 << 30;

/// Stack below which a walk stops following names ([`exhausted`]), so every
/// helper it calls still holds a tree of [`MAX_NESTING_DEPTH`].
const HEADROOM_BYTES: usize = PIPELINE_STACK_BYTES / 4;

/// What a walk that met [`exhausted`] reports at the node it stopped at.
pub const EXHAUSTED: &str =
    "the design nests too deeply here with the calls and declarations it names followed";

thread_local! {
    /// Address under which the current thread is [`exhausted`]; 0 on a thread
    /// that runs no pipeline.
    static FLOOR: Cell<usize> = const { Cell::new(0) };
}

/// The system could not start a pipeline thread, which is how a limit on
/// address space too small for its stack shows.
#[derive(Debug)]
pub struct PipelineStartError(std::io::Error);

impl fmt::Display for PipelineStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot start a thread with {} MiB of address space for its stack: {}",
            PIPELINE_STACK_BYTES >> 20,
            self.0
        )
    }
}

impl std::error::Error for PipelineStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Run `work` on a thread of its own with [`PIPELINE_STACK_BYTES`] of stack
/// and return its result. A panic in `work` continues in the caller.
pub fn run_pipeline<T: Send>(work: impl FnOnce() -> T + Send) -> Result<T, PipelineStartError> {
    run_on_stack(PIPELINE_STACK_BYTES, HEADROOM_BYTES, work).map_err(PipelineStartError)
}

/// [`run_pipeline`] on a stack of `bytes`, [`exhausted`] below `headroom`.
pub(crate) fn run_on_stack<T: Send>(
    bytes: usize,
    headroom: usize,
    work: impl FnOnce() -> T + Send,
) -> std::io::Result<T> {
    std::thread::scope(|scope| {
        let thread = std::thread::Builder::new()
            .name("llg-pipeline".to_owned())
            .stack_size(bytes)
            .spawn_scoped(scope, move || {
                FLOOR.set(stack_address().saturating_sub(bytes - headroom));
                work()
            })?;
        match thread.join() {
            Ok(value) => Ok(value),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

/// Whether a pipeline thread has too little stack left to follow another
/// name. Never true within the depth one source may nest.
#[inline]
pub fn exhausted() -> bool {
    stack_address() < FLOOR.get()
}

/// The address of a local of the caller. Every supported target's stack grows
/// towards lower addresses.
#[inline]
fn stack_address() -> usize {
    let local = 0_u8;
    std::ptr::addr_of!(local).addr()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of about 4 KiB per level; returns the level it stopped at.
    fn descend(level: usize) -> usize {
        let frame = std::hint::black_box([level; 512]);
        if exhausted() {
            return level;
        }
        let reached = descend(level + 1);
        std::hint::black_box(&frame);
        reached
    }

    #[test]
    fn a_walk_stops_before_the_reservation_ends() {
        const BYTES: usize = 8 << 20;
        const HEADROOM: usize = 2 << 20;
        let reached = run_on_stack(BYTES, HEADROOM, || descend(0)).expect("pipeline thread");
        let used = reached * 512 * std::mem::size_of::<usize>();
        assert!(
            used > BYTES / 4 && used < BYTES - HEADROOM,
            "{reached} levels"
        );
    }

    #[test]
    fn other_threads_are_never_exhausted() {
        assert!(!exhausted());
        run_pipeline(|| assert!(!exhausted())).expect("pipeline thread");
        assert!(!exhausted());
    }

    #[test]
    fn a_panic_continues_in_the_caller() {
        let panic = std::panic::catch_unwind(|| run_pipeline(|| panic!("in the pipeline")))
            .expect_err("the panic propagates");
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"in the pipeline"),
            "the payload is the pipeline's"
        );
    }
}

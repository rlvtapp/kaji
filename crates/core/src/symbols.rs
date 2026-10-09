//! Deterministic, opt-in symbol allocation: reserve everything, resolve once, emit.
//!
//! Callers choose language-appropriate preferred identifiers. Resolution uses stable
//! entity identities, never plugin order, and publishes an immutable lookup table.

mod allocator;
mod planning;

pub use allocator::*;
pub use planning::*;

#[cfg(test)]
mod plugin_tests;
#[cfg(test)]
mod tests;

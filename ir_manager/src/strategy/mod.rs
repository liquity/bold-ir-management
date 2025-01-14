//! All strategy related implementations and types.
//! Includes:
//! - Executable strategies
//! - Stable strategies
//! - Settings and data types
//! - Strategy runner

pub(crate) mod data;
pub(crate) mod run;
pub(crate) mod settings;
pub(crate) mod stale;
// As a safety measure, we want to know explicitly where we have access to the executable strategy.
pub(in crate::strategy) mod executable;

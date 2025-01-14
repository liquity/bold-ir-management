//! Liquity V2's Autonomous Interest Rate Management System

#![deny(clippy::unwrap_used)]
#![allow(clippy::missing_const_for_thread_local)]
#![warn(missing_docs)]

pub mod canister;
pub mod charger;
pub mod constants;
pub mod journal;
pub mod providers;
pub mod state;
pub mod strategy;
pub mod types;
pub mod utils;

pub use canister::IrManager;

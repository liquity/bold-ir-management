#![deny(clippy::unwrap_used)]
#![allow(clippy::missing_const_for_thread_local)]

mod canister;
mod charger;
mod constants;
mod journal;
mod providers;
mod state;
mod strategy;
mod types;
mod utils;
mod halt;

pub use canister::IrManager;

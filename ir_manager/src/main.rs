mod api;
mod canister;
mod charger;
mod evm_rpc;
mod gas;
mod signer;
mod state;
mod strategy;
mod timers;
mod types;
mod utils;
mod exchange;

use crate::canister::IrManager;

fn main() {
    let canister_e_idl = IrManager::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}

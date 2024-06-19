mod api;
mod canister;
mod evm_rpc;
mod state;
mod strategy;
mod types;
mod utils;
mod signer;

use crate::canister::IrManager;

fn main() {
    let canister_e_idl = IrManager::idl();
    let idl = candid::pretty::candid::compile(&canister_e_idl.env.env, &Some(canister_e_idl.actor));

    println!("{}", idl);
}

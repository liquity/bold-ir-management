#!/bin/bash
dfx deploy --ic
dfx canister call --ic ir_manager start '(1)'
dfx canister call --ic ir_manager assign_keys
dfx canister call --ic ir_manager start_timers '(record {rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0xE84251b93D9524E0d2e621Ba7dc7cb3579F997C0"; markets=vec {record {manager="0xE84251b93D9524E0d2e621Ba7dc7cb3579F997C0"; batch_managers=vec {"0xE84251b93D9524E0d2e621Ba7dc7cb3579F997C0"}; collateral_index=0; multi_trove_getter="0xE84251b93D9524E0d2e621Ba7dc7cb3579F997C0"}}; upfront_fee_period=7; rpc_url="https://www.liquity.org/"; collateral_registry="0xE84251b93D9524E0d2e621Ba7dc7cb3579F997C0"; strategies=vec {record {target_min=20}}})'
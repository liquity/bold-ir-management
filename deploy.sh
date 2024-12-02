#!/bin/sh
dfx canister start --ic ir_manager
dfx deploy --ic ir_manager --mode=reinstall

# COLLATERAL 1
dfx canister call --ic ir_manager mint_strategy 'record { key=0; manager="0x70fa06222e169329f7a2f386ed70ad69a61228a5"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x71d43e3ce3c8c593773dd9b843e9db949384adc7"; collateral_index=0; multi_trove_getter="0xd55dbe705404242cda541390361ce28ce7f50b95"; upfront_fee_period=604800; target_min=200000000000000000; collateral_registry="0xec0f62913efa850bf7fab03663ef7364afa9e481" }'
dfx canister call --ic ir_manager mint_strategy 'record { key=1; manager="0x71aca0d1c8ad87ced23d5816c2988d8d8a912ac3"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x71d43e3ce3c8c593773dd9b843e9db949384adc7"; collateral_index=1; multi_trove_getter="0xd55dbe705404242cda541390361ce28ce7f50b95"; upfront_fee_period=604800; target_min=200000000000000000; collateral_registry="0xec0f62913efa850bf7fab03663ef7364afa9e481" }'
dfx canister call --ic ir_manager mint_strategy 'record { key=2; manager="0xa8a2446696d9f3f49c39f020a5d6d34cbf3d81f4"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x71d43e3ce3c8c593773dd9b843e9db949384adc7"; collateral_index=2; multi_trove_getter="0xd55dbe705404242cda541390361ce28ce7f50b95"; upfront_fee_period=604800; target_min=200000000000000000; collateral_registry="0xec0f62913efa850bf7fab03663ef7364afa9e481" }'
dfx canister call --ic ir_manager set_batch_manager '(0, "0x9fAFA680723C09b7e06C7eC4e21A39377CCE8185")'
dfx canister call --ic ir_manager set_batch_manager '(1, "0x20a700e8c44067993905C4353472A4C69c26DD6c")'
dfx canister call --ic ir_manager set_batch_manager '(2, "0xF81b73cFfBb63811C95Ef0b2f25C057d23cBD053")'

dfx canister call --ic ir_manager start_timers
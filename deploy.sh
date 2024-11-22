#!/bin/sh
dfx deploy --ic ir_manager --mode=reinstall


# COLLATERAL 0
dfx canister call --ic ir_manager mint_strategy 'record { key=0; manager="0xe93d7ba2c636c32d38c2741b6715c77e2095694b"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x18f383fc8189ef1f2910f9fc1f9381eac5b11ac5"; collateral_index=0; multi_trove_getter="0x221c147fbe46a63b45ab223cc68bbfa4b1d75f97"; upfront_fee_period=7; target_min=20000000000000000; collateral_registry="0x0a39e30142735eefc7b28eed9572b59af41e1bd8" }'
dfx canister call --ic ir_manager set_batch_manager '(0, "0x9619c5aFDc1D9c45f0392228DABE4ef10D33e96C")'

# COLLATERAL 1
dfx canister call --ic ir_manager mint_strategy 'record { key=1; manager="0xc53ef07275d455a9433e3c90c6d24c740c7e6c61"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x18f383fc8189ef1f2910f9fc1f9381eac5b11ac5"; collateral_index=1; multi_trove_getter="0x221c147fbe46a63b45ab223cc68bbfa4b1d75f97"; upfront_fee_period=7; target_min=20000000000000000; collateral_registry="0x0a39e30142735eefc7b28eed9572b59af41e1bd8" }'
dfx canister call --ic ir_manager set_batch_manager '(1, "0x5B143E95c559a8f9aaF60ae227EC0Fe7c0573db8")'

# COLLATERAL 2
dfx canister call --ic ir_manager mint_strategy 'record { key=2; manager="0x1005178d3618424dfa2991a436e5f426288b3e2f"; rpc_principal=principal "7hfb6-caaaa-aaaar-qadga-cai"; hint_helper="0x18f383fc8189ef1f2910f9fc1f9381eac5b11ac5"; collateral_index=2; multi_trove_getter="0x221c147fbe46a63b45ab223cc68bbfa4b1d75f97"; upfront_fee_period=7; target_min=20000000000000000; collateral_registry="0x0a39e30142735eefc7b28eed9572b59af41e1bd8" }'
dfx canister call --ic ir_manager set_batch_manager '(2, "0xC0Aefb8A36D640e60312d577160B1C16F14257bb")'

dfx canister call --ic ir_manager start_timers
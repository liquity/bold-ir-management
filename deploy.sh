#!/bin/bash

# Load environment variables from .env file
export $(grep -v '^#' .env | xargs)

# Deploy the canister
dfx deploy --ic

# Call the start method
dfx canister call --ic ir_manager start '(1)'

# Call the assign_keys method
dfx canister call --ic ir_manager assign_keys

# Call the start_timers method with parameters from the .env file
dfx canister call --ic ir_manager start_timers "(
  record {
    rpc_principal = principal \"$RPC_PRINCIPAL\";
    hint_helper = \"$HINT_HELPER\";
    markets = vec {
      record {
        manager = \"$MANAGER\";
        batch_managers = vec { \"$BATCH_MANAGER\" };
        collateral_index = 0;
        multi_trove_getter = \"$MULTI_TROVE_GETTER\";
      }
    };
    upfront_fee_period = $UPFRONT_FEE_PERIOD;
    rpc_url = \"$RPC_URL\";
    collateral_registry = \"$COLLATERAL_REGISTRY\";
    strategies = vec {
      record { target_min = $TARGET_MIN }
    };
  }
)"

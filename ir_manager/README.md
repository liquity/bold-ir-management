# Interest Rate Manager Canister

## Deployment Guide

This guide outlines the steps required to deploy and configure the canister and associated batch managers.

### Step 1: Deploy the Canister on ICP

Use `dfx` to deploy the canister on the Internet Computer:

```bash
dfx deploy --network ic
```

Ensure you have the correct canister ID and the canister is successfully deployed before proceeding.

### Step 2: Mint Strategies in the Canister

Mint a new strategy using the `mint_strategy` function. Replace the placeholders in the command with your strategy-specific values.

```bash
dfx canister call ir_manager mint_strategy '(
    {
        key = <strategy_key_nat32>;
        target_min = <target_min_nat>;
        manager = "<trove_manager_address>";
        multi_trove_getter = "<multi_trove_getter_address>";
        collateral_index = <collateral_index_nat>;
        rpc_principal = principal "<rpc_canister_principal>";
        upfront_fee_period = <cooldown_period_in_seconds_nat>;
        collateral_registry = "<collateral_registry_address>";
        hint_helper = "<hint_helper_address>";
    }
)'
```

Immediately after minting each strategy, a new Ethereum Externally Owned Account (EOA) address is generated. This address should be used as the `batch_manager_eoa` of the strategy in the subsequent steps.

### Step 3: Deploy Batch Manager Contracts on Ethereum

For each strategy, deploy a batch manager contract using Foundry. Replace the placeholders with appropriate values and use the EOA address generated after minting the strategy as `batch_manager_eoa`.

```bash
forge create --rpc-url <rpc_url> --private-key <private_key> BatchManager --constructor-args \
    <batch_manager_eoa> \
    <trove_manager_address> \
    <borrower_operations_address> \
    <bold_token_address> \
    <weth_pricefeed_address> \
    <sorted_troves_address> \
    <min_interest_rate> \
    <max_interest_rate> \
    <current_interest_rate> \
    <fee> \
    <min_interest_rate_change_period> \
    <discount_rate>
```

Repeat this step for every strategy you need to configure.


### Step 4: Set Batch Manager Addresses for Strategies

Set the Ethereum batch manager address for each minted strategy using the `set_batch_manager` function.

```bash
dfx canister call ir_manager set_batch_manager '(
    <strategy_key_nat32>,
    "<batch_manager_address>",
    <current_rate_nat>
)'
```

Ensure this is done for all strategies minted in Step 2.


### Step 5: Start Timers

Initialize the timers for strategy execution and maintenance tasks.

```bash
dfx canister call ir_manager start_timers
```

This step ensures all system tasks are set up and ready to execute.


### Step 6: Blackhole the Canister

Once all configurations are complete and the canister is operational, make it immutable by blackholing it. This step ensures that no further updates or changes can be made.
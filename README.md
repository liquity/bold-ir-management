# BOLD Interest Rate Manager

## Overview

The BOLD Interest Rate (IR) Manager is a Rust-based canister designed to work in tandem with the Batch Manager Solidity smart contract. Together, they automatically adjust troves' interest rates for the Liquity Protocol v2. This automated adjustment process leverages the Internet Computer's tECDSA signatures to create Ethereum Mainnet Externally Owned Accounts (EOAs) and execute rate adjustment transactions via these accounts.

The BOLD IR Manager can manage multiple strategies, each linked to different Batch Manager contracts and EOAs. It periodically checks the state of sorted troves every hour and will initiate an update transaction if any of the rate adjustment conditions—whether to increase or decrease the rates—are met.

## Calculations

Whenever a strategy is being executed, the following calculations are performed:

- Increase check:

    ![](./assets/update_condition.png)

- First decrease check:

    ![](./assets/first_decrease_condition.png)

- Second decrease check:

    ![](./assets/second_decrease_condition.png)

- New rate calculation:

    ![](./assets/new_rate.png)

- Definitions:

    ![](./assets/definitions.png)

    ![](./assets/maximumRedemptionCollateral.png)

## Recharge Flow

The diagram below illustrates how the canister recharges itself automatically by providing financial incentives to external participants. These participants can supply the protocol with ETH and Cycles in exchange for BOLD and ckETH, respectively.

![Recharge Flow](./assets/Recharge_flow.png)

### How It Works

#### How the New Rate is Set

1. **Periodic Trove Checks**: The BOLD IR Manager periodically checks the state of troves every hour. It evaluates whether the interest rates need adjustment based on predefined conditions for increasing or decreasing rates.

2. **tECDSA Signature Generation**: If a rate adjustment is needed, the IR Manager generates a tECDSA signature to authorize a transaction on the Ethereum network. This signature is used to create an Ethereum transaction that adjusts the interest rates as required.

3. **Executing Transactions via EOAs**: The authorized transaction is executed via the EOAs on the Ethereum Mainnet, which have been set up by the IR Manager. This ensures that the rate adjustments are carried out efficiently and securely on the Ethereum blockchain.

#### How the Canister and EOAs Keep Their Gas Tokens as ETH and Cycles

1. **ckETH Minting & Transfer**: The process begins with the Ethereum side where ETH is sent to the Batch Manager, which is responsible for collecting accrued fees as BOLD. The Strategy EOA then mints ckETH using the sent ETH. However, the ckETH minting checks occur only every 24 hours. New ckETH will only be minted if the canister's ckETH balance is below a specific threshold, and it will mint a predefined amount. This is necessary because minting ckETH takes approximately 20 minutes to be picked up by the Internet Computer's ckETH Minter canister. The goal is to ensure that there is always enough ckETH available to provide to users sending cycles, if the canister is accepting cycles.

2. **Cycle Acceptance Check**: The decision to accept cycles is determined by the canister's cycles balance. If the balance falls below a certain threshold, the canister will accept cycles and mint ckETH in response.

3. **Maintaining Gas Token Reserves**: Both the canister and EOAs must maintain sufficient reserves of gas tokens (ETH on Ethereum and cycles on the Internet Computer) to continue operating efficiently. The Batch Manager ensures that the necessary ETH is available by managing the accrued fees and supplying the EOAs with enough ETH. Similarly, the canister checks its cycle balance and recharges itself as needed to sustain its operations.

4. **Arbitrage Opportunity**: An arbitrageur can send ETH to receive discounted BOLD or send cycles to receive ckETH. This mechanism incentivizes the continuous supply of ETH and Cycles to the protocol, ensuring that it remains self-sustaining.

## Canister Methods

The BOLD IR Manager canister exposes several key methods for interaction:

- **`start`**: Initializes the canister's strategy data state with placeholder values that allow for tECDSA key generation.
- **`assign_keys`**: Initializes public keys for the strategies' EOAs to interact securely with Ethereum.
- **`start_timers`**: Accepts `InitArgs` as input to start all relevant timers for interest rate adjustments, ckETH balance checks, and more. This method allows for customizable configuration on initialization, including the specification of markets, RPC settings, collateral registry, and strategies. It also blackholes the canister (removes all controllers)
- **`swap_cketh`**: Allows any caller to send cycles for recharging purposes and receive ckETH in return. This method returns a `SwapResponse` that contains details about the transaction, such as the amount of ether returned and cycles accepted.
- **`get_strategies`**: Retrieves all active strategies along with their corresponding EOAs and related data, such as the latest rate and the last update time.

## Technical Details: tECDSA Signatures

### What are tECDSA Signatures?

Threshold ECDSA (tECDSA) is a cryptographic technique that allows multiple parties to collaboratively generate a digital signature without ever revealing the private key. This is particularly useful in decentralized environments where the security of the private key is paramount.

### How tECDSA is Used in the BOLD IR Manager

In the BOLD IR Manager, tECDSA signatures are used to authorize Ethereum transactions from the canister without the need to expose or directly handle the private keys. This process involves the following steps:

1. **Distributed Key Generation (DKG)**: The Internet Computer's canisters participate in a distributed key generation process, where a private key is split into shares among multiple nodes.

2. **Signature Generation**: When a transaction needs to be signed, these nodes work together to generate a tECDSA signature using their key shares. The full private key is never reconstructed, enhancing security.

3. **Transaction Execution**: The tECDSA signature is then used to sign Ethereum transactions that adjust interest rates or manage ETH balances. This ensures that the transactions are secure and that the private key is never compromised.

4. **Security and Decentralization**: By utilizing tECDSA, the BOLD IR Manager can maintain a high level of security while operating in a decentralized manner. The process eliminates the need for a single point of failure, as the private key is never stored in one place.

## Deployment Details

The BOLD IR Manager canister is not upgradable and will be blackholed upon starting. This means that all controllers will be removed, making the canister immutable. Any user may choose to deploy their own Batch Manager contract and IR Manager canister to handle their troves. However, the Liquity Protocol will offer several pre-configured strategies for users to choose from.

## Summary

The BOLD IR Manager and Batch Manager together ensure the automated adjustment of interest rates within the Liquity Protocol v2 by leveraging decentralized infrastructure and providing financial incentives for participation. This setup aims to maintain the protocol's long-term stability and sustainability, while allowing for flexibility in user deployment and strategy selection. The integration of tECDSA signatures further enhances the security and decentralization of the entire process.

# BOLD Interest Rate Manager

## Overview

The **BOLD Interest Rate (IR) Manager** is an innovative Rust-based canister that integrates seamlessly with the **Batch Manager** Solidity smart contract to automate the adjustment of interest rates for troves within the **Liquity Protocol v2**. This solution utilizes the Internet Computer’s threshold ECDSA (tECDSA) signatures to manage Ethereum Mainnet Externally Owned Accounts (EOAs) and securely execute rate adjustment transactions, offering a decentralized and secure way to maintain optimal interest rates across multiple strategies.

The BOLD IR Manager is capable of managing multiple interest rate adjustment strategies, each associated with different Batch Manager contracts and EOAs. It continuously monitors the state of sorted troves, checking every hour whether the conditions for rate adjustment—either an increase or a decrease—are met. If the conditions are met, the manager initiates a transaction to adjust the rates, maintaining the system’s stability and ensuring competitive borrowing costs.

## Calculations

In Liquity V2, borrowers incur the following fees:

- [Interest rate](https://github.com/liquity/bold/blob/main/README.md#borrowing-and-interest-rates): a recurrent rate set by the borrower and charged on their current debt
- [Premature adjustment fee](https://github.com/liquity/bold/blob/main/README.md#premature-adjustment-fees): a one-off fee corresponding to 1 week of the average interest rate of the respective collateral market, charged on the debt whenever the borrower adjusts their interest rate within less than 7 days since the last adjustment ("cooling off period"). The same fee is charged when a new Trove is opened or when its debt is increased ([see](https://github.com/liquity/bold/blob/main/README.md#upfront-borrowing-fees))

Note: In addition to these fees, borrowers delegating to a batch manager may also be charged a management fee; [see](https://github.com/liquity/bold/blob/main/README.md#batch-management-fee)

An optimal interest rate strategy should minimize the costs of borrowing by striking a balance between the interest rate and its adjustment frequency as well as the redemption risk ([see](https://github.com/liquity/bold/blob/main/README.md#bold-redemptions) on redemptions in Liquity V2).

To that end, for each defined strategy, the autonomous management system targets a specific debt percentage to be in front (i.e. to be redeemed first) of all the Troves participating in the strategy. To determine the debt in front, the system calculates the percentage of redemptions hitting the respective collateral branch and uses it to loop over the list of Troves in the branch, ordered by interest rate from lowest to highest.

The base debt D is a parameter preset for each strategy and determines the target range for the debt in front, along with tolerance margins for up and down deviations defined as system-wide constants. The system thus aims to adjust the interest rate to achieve the mid point of the target range when the debt in front gets out of range, by increasing or decreasing the rate as needed.

In times of elevated redemption risk, the target debt range is increased (rescaled) to create a larger buffer for subsequent redemptions and the possibility of other borrowers increasing their own interest rates. The current redemption fee is used as a proxy for recent redemption activity and a metric to predict further redemptions.

Based on these considerations, the system checks whether the conditions for increasing the interest rate are met using this formula:

![Increase Check](./assets/update_condition.png)

When the debt in front becomes larger than the upper bound of the target range, the interest rate is adjusted if both of the two following conditions hold:

![First Decrease Check](./assets/first_decrease_condition.png)

![Second Decrease Check](./assets/second_decrease_condition.png)

The second condition aims to keep the costs of premature adjustments low, by only performing the rate reduction if the last adjustment happened less than seven days ago, or if the prospective savings from the lowered interest rate until the end of the cooling off period exceed the adjustment fee. Note that the adjustment fee equals the (size-weighted) average interest paid by all borrowers in the same collateral branch as the borrowers participating in the given strategy ([see](https://github.com/liquity/bold/blob/main/README.md#premature-adjustment-fees)).

The new interest rate is calculated to achieve a debt in front corresponding to the mid point of the target range by looping over.

![New Rate Calculation](./assets/new_rate.png)

Definitions of the terms and parameters used in the rate adjustment calculations:

![Definitions](./assets/definitions.png)

![Maximum Redemption Collateral](./assets/maximumRedemptionCollateral.png)

![TargetAmount](./assets/targetAmount.png)

## Recharge Flow

The BOLD IR Manager includes a self-sustaining recharge mechanism. The diagram below illustrates how the canister automatically recharges itself by offering financial incentives to external participants. These participants can supply the protocol with ETH and Cycles in exchange for BOLD tokens and ckETH, respectively.

![Recharge Flow](./assets/diagrams/Arbitrage.png)

### Recharge Mechanism Explanation

1. **ETH Supply and ckETH Minting**: Participants send ETH to the Batch Manager contract on Ethereum. The Strategy EOA uses this ETH to mint ckETH, which can then be transferred back to the Internet Computer. The system periodically checks (every 24 hours) if the canister’s ckETH balance falls below a specific threshold, prompting a new minting of ckETH.

2. **Cycle Acceptance and Exchange**: If the canister's cycle balance is low, it will accept cycles from participants in exchange for ckETH, maintaining a stable supply of resources to continue its operations.

3. **Financial Incentives and Arbitrage Opportunities**: The system creates arbitrage opportunities where participants can supply ETH to receive discounted BOLD tokens or send Cycles to receive ckETH, ensuring a continuous supply of essential resources to sustain the protocol.

## Rate Adjustment Flow

The rate adjustment process follows a specific flow to ensure that interest rates are updated efficiently and securely. The diagram below illustrates this flow:

![Rate Adjustment Flow Diagram](./assets/diagrams/RateAdjustment.png)

### Rate Adjustment Process Details

1. **Monitoring Trove States**: The IR Manager periodically checks the troves’ states to evaluate the need for rate adjustments. If any conditions for rate modification (increase or decrease) are met, it prepares to initiate a transaction.

2. **Generating tECDSA Signatures**: When an adjustment is necessary, the IR Manager generates a tECDSA signature, a secure method of authorizing transactions on the Ethereum network without exposing the private keys.

3. **Executing Adjustments**: Using the generated signature, the IR Manager executes the required transactions through the EOAs on the Ethereum Mainnet. This approach ensures that the interest rates are adjusted in a decentralized, secure, and automated manner.

## Concurrency Across Strategies

The BOLD IR Manager is designed to handle multiple strategies concurrently, ensuring that interest rates are adjusted in real-time across various protocols and markets. The diagram below illustrates how concurrency is managed:

![Concurrency Across Strategies Diagram](./assets/diagrams/Concurrency.png)

### Internet Computer's Concurrency

On the Internet Computer, concurrency is managed by leveraging the protocol’s ability to handle asynchronous messages. This allows the system to yield control between different processes and resume them later, ensuring efficient resource management and smooth operation.

![IC Concurrency](./assets/diagrams/InternetComputerConcurrency.png)

## How It Works

### How the New Rate is Set

1. **Periodic Trove Checks**: Every hour, the BOLD IR Manager checks the state of troves. It evaluates whether interest rates need to be adjusted based on predefined conditions, including market demand, collateral ratios, and protocol-defined parameters.

2. **tECDSA Signature Generation**: If a rate adjustment is deemed necessary, the IR Manager generates a tECDSA signature. This signature is used to authorize the transaction on the Ethereum network securely.

3. **Executing Transactions via EOAs**: The IR Manager uses the generated tECDSA signature to sign and execute transactions via EOAs on the Ethereum Mainnet. This method ensures that adjustments are made without exposing private keys or compromising security.

### Gas Token Management for Canister and EOAs

1. **ckETH Minting & Transfer**: On the Ethereum side, ETH is sent to the Batch Manager, which manages accrued fees as BOLD tokens. The Strategy EOA mints ckETH using this ETH. Minting occurs every 24 hours if the canister's ckETH balance is below a specified threshold, ensuring a sufficient supply for protocol operations.

2. **Cycle Acceptance Check**: The canister accepts cycles when its balance falls below a certain level. It exchanges cycles for ckETH to maintain adequate operational reserves.

3. **Maintaining Gas Token Reserves**: The Batch Manager ensures enough ETH is available by managing accrued fees and supplying the EOAs with necessary resources. Simultaneously, the canister checks its cycle balance and recharges as needed.

4. **Arbitrage Opportunities**: Arbitrage participants can send ETH for discounted BOLD or cycles for ckETH, incentivizing the continuous supply of ETH and Cycles to the protocol.

## Canister Methods

The BOLD IR Manager canister provides several methods to enable interaction and configuration:

- **`mint_strategy`**: Creates a new strategy with the given parameters and returns the EOA for that strategy key.
- **`set_batch_manager`**: Updates a strategy's batch manager, if unset.
- **`start_timers`**: Starts the canister's timers.
- **`swap_cketh`**: Allows participants to exchange cycles for ckETH, providing details on the transaction (amount of ETH returned, cycles accepted).
- **`get_strategies`**: Fetches all active strategies, associated EOAs, and relevant data such as the current rate and last update time.

## Technical Details: tECDSA Signatures

### What are tECDSA Signatures?

Threshold ECDSA (tECDSA) is a cryptographic method that enables multiple parties to jointly generate a digital signature without ever revealing the private key. This technique enhances security in decentralized environments by eliminating single points of failure.

### Usage in the BOLD IR Manager

In the BOLD IR Manager, tECDSA signatures play a crucial role in authorizing Ethereum transactions:

1. **Distributed Key Generation (DKG)**: The Internet Computer canisters participate in generating a private key that is divided among multiple nodes.

2. **Signature Generation**: When a transaction requires signing, the nodes collaborate to generate a tECDSA signature using their shares of the private key. The private key is never fully reconstructed, enhancing security.

3. **Transaction Execution**: The tECDSA signature authorizes Ethereum transactions for rate adjustments or ETH management, ensuring that private keys remain secure.

4. **Security and Decentralization**: The use of tECDSA ensures a high level of security and decentralization, minimizing the risk of a single point of failure and preventing private key exposure.

## Deployment Details

The BOLD IR Manager canister is designed to be immutable and non-upgradable. Upon initialization, it will be "blackholed," meaning all controllers are removed to make the canister immutable. Users have the flexibility to deploy their own Batch Manager contracts and IR Manager canisters, though pre-configured strategies will also be available through the Liquity Protocol.

## Security Requirements

The IR Manager Rust canister and the Batch Manager Solidity contract are designed for trustlessness, with no administrative or governance privileges:

- **No Governance/Admin Access**: The canister and contract logic is fixed upon deployment, covering all aspects of strategy, rate adjustment calculations, and more.

- **Minimized External Update Methods**: Only three external permissioned update methods are exposed to facilitate deployment and initialization. After deployment, these methods are locked, and the canister is blackholed to prevent unauthorized access. The canister also provides an external update method for Cycles<>ckETH arbitrage, while the Batch Manager contract exposes two functions: one for Ether<>BOLD arbitrage and another for proxying rate adjustment calls from strategy EOAs.

- **Logs and Retry Mechanisms**: The canister includes retry loops to handle errors, reporting them in logs and reattempting operations if necessary. It is designed to minimize traps and panics, ensuring reliable autonomous operation.

## Summary

The BOLD IR Manager and Batch Manager collaboratively ensure the automated adjustment of interest rates within the Liquity Protocol v2 by leveraging decentralized infrastructure and providing financial incentives for participation. The setup is designed to maintain long-term protocol stability and sustainability while offering flexibility in deployment and strategy selection. The integration of tECDSA signatures further enhances security and decentralization, making the entire process robust and resistant to single points of failure.

By combining cutting-edge cryptography with a decentralized approach, the BOLD IR Manager provides a secure, efficient, and automated solution for managing interest rates in DeFi, enabling the Liquity Protocol v2 to remain competitive and resilient in an ever-evolving financial landscape.

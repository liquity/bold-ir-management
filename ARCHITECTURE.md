# Architecture

This document provides a high-level overview of the architecture of the BOLD Interest Rate Manager project, outlining the structure of the repository and the responsibilities of each major component.

## Project Structure

### 1. `assets/`
- **Contents**: Contains visual assets, such as diagrams and images.
- **Purpose**: This directory holds visual aids used in documentation, like the `Recharge_flow.png`, which illustrates the recharge flow of the system.

### 2. `build.sh`
- **Purpose**: A shell script responsible for automating the build process of the project. It typically compiles the code, runs tests, and prepares the project for deployment.

### 3. `Cargo.toml`
- **Purpose**: The main configuration file for Rust projects. It specifies the dependencies, project metadata, and build settings for the `ir_manager` Rust canister.

### 4. `contracts/`
- **Purpose**: This directory contains the Solidity smart contracts used in the project. These contracts are integral to managing interactions on the Ethereum blockchain, particularly within the Liquity Protocol v2 ecosystem.

#### Key Subdirectories and Files:
- **`foundry.toml`**: Configuration file for the Foundry toolchain, which is used for Solidity development and testing.
- **`remappings.txt`**: Defines import remappings, allowing the project to resolve Solidity dependencies correctly.
- **`script/`**: Contains deployment and interaction scripts, such as `Counter.s.sol`, which may be used for testing or initializing smart contracts.
  
- **`src/`**
    - **Purpose**: The primary source directory for the Solidity smart contracts.
  
    - **`BatchManager.sol`**: The main contract responsible for managing batches of operations related to the adjustment of interest rates and handling of Ethereum assets within the protocol.
    - **`Dependencies/`**: Contains external contract interfaces like `AggregatorV3Interface.sol`, which are dependencies required by the main contracts.
    - **`Interfaces/`**: Houses interfaces that define interactions with Liquity v2 smart contracts. These interfaces include contracts such as `IActivePool.sol`, `IBoldToken.sol`, `ITroveManager.sol`, and others that facilitate communication between the Batch Manager and the Liquity Protocol.
    - **`Types/`**: Defines custom Solidity types used within the contracts, such as `BatchId.sol`, `LatestBatchData.sol`, `LatestTroveData.sol`, and `TroveChange.sol`.

### 5. `dfx.json`
- **Purpose**: Configuration file for the DFINITY Internet Computer development environment. It defines settings for canister deployment, building, and managing Internet Computer projects.

### 6. `fix_and_fmt.sh`
- **Purpose**: A shell script used to format and fix code issues in the project.

### 7. `ir_manager/`
- **Purpose**: This directory contains the source code and configuration for the BOLD IR Manager canister. This canister runs on the Internet Computer and is responsible for managing interest rates, interacting with Ethereum, and handling other core functions.

#### Key Files:
- **`candid.did`**: Defines the interface of the canister in the Candid language, which is used for specifying and interacting with Internet Computer services.
- **`Cargo.toml`**: The configuration file for the Rust project, specifying dependencies and settings specific to the IR Manager canister.
  
- **`src/`**
    - **Purpose**: Contains the Rust source code for the IR Manager canister.

    - **`canister.rs`**: The main file for canister-related logic, defining the core functionality and API exposed by the IR Manager canister.
    - **`charger.rs`**: Manages the logic for recharging the canister's cycles and ckETH balance, ensuring that the canister has sufficient resources for ongoing operations.
    - **`evm_rpc.rs`**: Handles communication with Ethereum via Remote Procedure Calls (RPC), facilitating transactions, balance checks, and other Ethereum interactions.
    - **`exchange.rs`**: Contains DFINITY's exchange rates canister's types.
    - **`gas.rs`**: Manages the gas estimation and usage for transactions.
    - **`main.rs`**: The entry point for the canister, initializing the canister state and starting the main event loop.
    - **`signer.rs`**: Implements tECDSA signature generation and signing logic, allowing the canister to securely authorize Ethereum transactions.
    - **`state.rs`**: Manages the internal state of the canister, including tracking strategies and configuration settings.
    - **`strategy.rs`**: Defines the strategies for adjusting interest rates, including the logic for evaluating and executing these strategies.
    - **`types.rs`**: Defines custom types used within the Rust code, providing clear and structured data handling across the canister's logic.
    - **`utils.rs`**: A utility module that provides common helper functions used across the canister's codebase.

### 8. `README.md`
- **Purpose**: Provides an overview of the project, including the purpose, functionality, and usage instructions for the BOLD Interest Rate Manager. This file serves as the primary documentation for users and developers interacting with the project.

## Summary

The BOLD Interest Rate Manager project is organized into distinct directories and files that each play a critical role in the operation and management of interest rates within the Liquity Protocol v2. The `contracts` directory handles the Ethereum smart contract logic, while the `ir_manager` directory manages the Internet Computer canister responsible for interacting with these contracts and performing automated tasks. Scripts and configuration files like `build.sh`, `dfx.json`, and `Cargo.toml` ensure that the project is easy to build, deploy, and maintain. This architecture allows for a seamless integration between decentralized finance on Ethereum and the powerful computation capabilities of the Internet Computer.
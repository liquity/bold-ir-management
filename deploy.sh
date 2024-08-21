#!/bin/bash

# Define color codes
INFO_COLOR='\033[0;36m'  # Cyan for information
SUCCESS_COLOR='\033[0;32m'  # Green for success
ERROR_COLOR='\033[0;31m'  # Red for errors
RESET_COLOR='\033[0m'  # Reset color
BREAK_LINE="==================================================="

# Function to run the script logic
run_script() {
    # Step 1: Load environment variables from the .env file
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Loading environment variables from .env file...${RESET_COLOR}"
    export $(grep -v '^#' .env | xargs)
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to load environment variables!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] Environment variables loaded successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 2: Deploy the canister to the Internet Computer (IC)
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Deploying the canister to the Internet Computer (IC)...${RESET_COLOR}"
    dfx deploy --ic
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Canister deployment failed!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] Canister deployed successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 3: Start the 'ir_manager' canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Starting the 'ir_manager' canister...${RESET_COLOR}"
    dfx canister --ic start ir_manager
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to start the 'ir_manager' canister!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'ir_manager' canister started successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 4: Call the 'start' method on the canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Calling the 'start' method on the canister...${RESET_COLOR}"
    dfx canister call --ic ir_manager start '(1)'
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to call 'start' method!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'start' method called successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 5: Call the 'assign_keys' method on the canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Calling the 'assign_keys' method on the canister...${RESET_COLOR}"
    dfx canister call --ic ir_manager assign_keys
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to call 'assign_keys' method!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'assign_keys' method called successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 6: Extract the strategy EOA by calling 'get_strategy_address' and parsing the result
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Retrieving the strategy EOA...${RESET_COLOR}"
    strategy_response=$(dfx canister call --ic ir_manager get_strategy_address '(0)')
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to retrieve strategy EOA!${RESET_COLOR}"
      exit 1
    fi
    STRATEGY_EOA=$(echo "$strategy_response" | sed -n 's/(opt "\(.*\)")/\1/p')
    echo -e "> ${SUCCESS_COLOR}[INFO] Strategy EOA retrieved: $STRATEGY_EOA${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

    # Step 7: Optionally deploy batch manager or use existing
    if [ -z "$BATCH_MANAGER" ]; then
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
        echo -e "> [INFO] Deploying batch managers using Forge...${RESET_COLOR}"
        cd contracts
        forge_output=$(forge script script/BatchManager.s.sol:BatchManagerDeployer --sig "run(address,address,address,address,address)" --rpc-url "$RPC_URL" --broadcast --verify -vvvv $STRATEGY_EOA $MANAGER $BORROWER_OPERATIONS $BOLD_TOKEN $WETH_PRICE_FEED)
        cd ..

        # Extract contract address from Forge output
        BATCH_MANAGER=$(echo "$forge_output" | grep -oP 'BatchManager@0x\K[0-9a-fA-F]{40}')
        if [ -z "$BATCH_MANAGER" ]; then
          echo -e "> ${ERROR_COLOR}[ERROR] Failed to extract Batch Manager contract address!${RESET_COLOR}"
          exit 1
        fi

        echo -e "> ${SUCCESS_COLOR}[INFO] Batch Manager deployed successfully at address: 0x$BATCH_MANAGER${RESET_COLOR}"
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    else
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
        echo -e "> [INFO] Using existing Batch Manager at address: 0x$BATCH_MANAGER${RESET_COLOR}"
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    fi

    # Step 8: Call the 'start_timers' method with parameters from the .env file
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Calling the 'start_timers' method on the canister with parameters from the .env file...${RESET_COLOR}"
    dfx canister call --ic ir_manager start_timers "(
      record {
        rpc_principal = principal \"$RPC_PRINCIPAL\";
        hint_helper = \"$HINT_HELPER\";
        markets = vec {
          record {
            manager = \"$MANAGER\";
            batch_managers = vec { \"0x$BATCH_MANAGER\" };
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
    if [ $? -ne 0 ]; then
      echo -e "> ${ERROR_COLOR}[ERROR] Failed to call 'start_timers' method!${RESET_COLOR}"
      exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'start_timers' method called successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
}

# Ask if BatchManager is already deployed
echo -n "> Do you have an already deployed Batch Manager? (y/n): "
read answer

if [ "$answer" == "y" ]; then
    echo -n "> Enter the Batch Manager address: "
    read BATCH_MANAGER

    # Check if the input starts with "0x" and strip it if present
    if [[ "$BATCH_MANAGER" == 0x* ]]; then
        BATCH_MANAGER="${BATCH_MANAGER:2}"
    fi

    echo "> Stripped Batch Manager address: $BATCH_MANAGER"
else
    BATCH_MANAGER=""
fi

# Main loop
while true; do
    run_script

    # Ask the user if they want to retry or quit
    echo -e "\n> ${INFO_COLOR}Script finished.${RESET_COLOR}"
    echo -n "> Press 'r' to retry, 'q' to quit: "
    read user_input

    if [ "$user_input" == "q" ]; then
        echo -e "> ${SUCCESS_COLOR}Quitting...${RESET_COLOR}"
        exit 0
    elif [ "$user_input" == "r" ]; then
        echo -e "> ${INFO_COLOR}Retrying...${RESET_COLOR}"
    else
        echo -e "> ${ERROR_COLOR}Invalid input. Please press 'r' to retry or 'q' to quit.${RESET_COLOR}"
    fi
done

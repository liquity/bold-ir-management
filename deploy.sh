#!/bin/bash

# ASCII Art because why not
clear
echo " _     ___ ___  _   _ ___ _______   __    __     ______  "
echo "| |   |_ _/ _ \| | | |_ _|_   _\ \ / /    \ \   / /___ \ "
echo "| |    | | | | | | | || |  | |  \ V /      \ \ / /  __) |"
echo "| |___ | | |_| | |_| || |  | |   | |        \ V /  / __/ "
echo "|_____|___\__\_\\\___/|___| |_|   |_|         \_/  |_____|"
echo ""

# Define color codes
INFO_COLOR='\033[0;36m'  # Cyan for information
SUCCESS_COLOR='\033[0;32m'  # Green for success
ERROR_COLOR='\033[0;31m'  # Red for errors
RESET_COLOR='\033[0m'  # Reset color
BREAK_LINE="==================================================="

batch_managers=()
target_mins=()
eoa_addresses=()
generate_troves="n"

# Get the canister ID for 'ir_manager'
IR_MANAGER_CANISTER_ID=$(dfx canister --ic id ir_manager)
if [ $? -ne 0 ]; then
    echo -e "> ${ERROR_COLOR}[ERROR] Failed to retrieve canister ID for 'ir_manager'!${RESET_COLOR}"
    exit 1
fi
echo -e "> ${SUCCESS_COLOR}[INFO] Retrieved canister ID for 'ir_manager': $IR_MANAGER_CANISTER_ID${RESET_COLOR}"

# Ask if the user wants to generate troves for batch managers
echo -n "> Do you want to generate troves for batch managers? (y/n): "
read generate_troves

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
    deploy_output=$(dfx deploy --ic)
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Canister deployment failed!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] Canister deployed successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    
    # Step 3: Start the 'ir_manager' canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Starting the 'ir_manager' canister...${RESET_COLOR}"
    dfx canister --ic start "$IR_MANAGER_CANISTER_ID"
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Failed to start the 'ir_manager' canister!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'ir_manager' canister started successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    
    # Step 4: Call the 'start' method on the canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    num_batch_managers=${#target_mins[@]}
    echo -e "> [INFO] Calling the 'start' method on the canister with value: $num_batch_managers...${RESET_COLOR}"
    dfx canister call --ic "$IR_MANAGER_CANISTER_ID" start "($num_batch_managers)"
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Failed to call 'start' method!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'start' method called successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    
    # Step 5: Call the 'assign_keys' method on the canister
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Calling the 'assign_keys' method on the canister...${RESET_COLOR}"
    dfx canister call --ic "$IR_MANAGER_CANISTER_ID" assign_keys
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Failed to call 'assign_keys' method!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] 'assign_keys' method called successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    
    # Step 6: Extract the strategy EOAs for each batch manager
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    for (( i=0; i<${#target_mins[@]}; i++ )); do
        echo -e "> [INFO] Retrieving the strategy EOA for Batch Manager $((i+1))...${RESET_COLOR}"
        strategy_response=$(dfx canister call --ic "$IR_MANAGER_CANISTER_ID" get_strategy_address "($i)")
        if [ $? -ne 0 ]; then
            echo -e "> ${ERROR_COLOR}[ERROR] Failed to retrieve strategy EOA for Batch Manager $((i+1))!${RESET_COLOR}"
            exit 1
        fi
        EOA=$(echo "$strategy_response" | sed -n 's/(opt "\(.*\)")/\1/p')
        eoa_addresses+=("$EOA")
        echo -e "> ${SUCCESS_COLOR}[INFO] Strategy EOA for Batch Manager $((i+1)) retrieved: $EOA${RESET_COLOR}"
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    done
    
    # Step 7: Optionally deploy batch manager or use existing
    if [ -z "$BATCH_MANAGER" ]; then
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
        echo -e "> [INFO] Deploying batch managers using Forge...${RESET_COLOR}"
        cd contracts
        for (( i=0; i<${#target_mins[@]}; i++ )); do
            forge_output=$(forge create --rpc-url "$RPC_URL" \
                --constructor-args "${eoa_addresses[i]}" "$MANAGER" "$BORROWER_OPERATIONS" "$BOLD_TOKEN" "$WETH_PRICE_FEED" "$SORTED_TROVES" "$MIN_INTEREST_RATE" "$MAX_INTEREST_RATE" "$CURRENT_RATE" "$ANNUAL_MANAGEMENT_FEE" "$MIN_INTEREST_RATE_CHANGE_PERIOD" "$DISCOUNT_RATE_BATCH_MANAGER" \
                --private-key "$PRIVATE_KEY" \
                --verify \
                --verifier sourcify \
                --verifier-url "$VERIFIER_URL" \
            src/BatchManager.sol:BatchManager)
            echo "$forge_output"
            BATCH_MANAGER=$(echo "$forge_output" | grep -oP 'Deployed to: (0x[0-9a-fA-F]{40})' | sed 's/Deployed to: //' | head -n 1)
            batch_managers+=("$BATCH_MANAGER")
            if [ -z "$BATCH_MANAGER" ]; then
                echo -e "> ${ERROR_COLOR}[ERROR] Failed to extract Batch Manager contract address for Batch Manager $((i+1))!${RESET_COLOR}"
                exit 1
            fi
            
            echo "> Extracted Batch Manager address: $BATCH_MANAGER"
            echo -e "> ${SUCCESS_COLOR}[INFO] Batch Manager deployed successfully at address: $BATCH_MANAGER${RESET_COLOR}"
            
            # Generate troves if requested
            if [ "$generate_troves" == "y" ]; then
                echo -e "> [INFO] Generating troves for Batch Manager at address: $BATCH_MANAGER...${RESET_COLOR}"
                BATCH_MANAGER="$BATCH_MANAGER" forge script OpenTroves --rpc-url "$RPC_URL" --broadcast -vvvv --private-key "$PRIVATE_KEY"
                if [ $? -ne 0 ]; then
                    echo -e "> ${ERROR_COLOR}[ERROR] Failed to generate troves for Batch Manager at address: $BATCH_MANAGER!${RESET_COLOR}"
                    exit 1
                fi
                echo -e "> ${SUCCESS_COLOR}[INFO] Troves generated successfully for Batch Manager at address: $BATCH_MANAGER.${RESET_COLOR}"
            fi
            
            echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
        done
        cd ..
    else
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
        echo -e "> [INFO] Using existing Batch Manager at address: $BATCH_MANAGER${RESET_COLOR}"
        echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    fi
    
    # Step 8: Call the 'start_timers' method with parameters for all batch managers
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    echo -e "> [INFO] Calling the 'start_timers' method on the canister with all Batch Managers and corresponding Target Mins...${RESET_COLOR}"
    
    # Prepare the batch managers vector
    batch_managers_vec=$(printf "\"%s\"; " "${batch_managers[@]}")
    batch_managers_vec=${batch_managers_vec%, }  # Remove the trailing comma and space
    echo $batch_managers_vec
    
    # Prepare the strategies vector dynamically
    strategies_vec=""
    for target_min in "${target_mins[@]}"; do
        strategies_vec+="record { target_min = $target_min }; "
    done
    strategies_vec=${strategies_vec%, }  # Remove the trailing comma and space
    
    echo $strategies_vec
    
    # Call the 'start_timers' method with the dynamically prepared vectors
    dfx canister call --ic "$IR_MANAGER_CANISTER_ID" start_timers "(
  record {
    rpc_principal = principal \"$RPC_PRINCIPAL\";
    hint_helper = \"$HINT_HELPER\";
    markets = vec {
      record {
        manager = \"$MANAGER\";
        batch_managers = vec { $batch_managers_vec };
        collateral_index = 0;
        multi_trove_getter = \"$MULTI_TROVE_GETTER\";
      }
    };
    upfront_fee_period = $UPFRONT_FEE_PERIOD;
    rpc_url = \"$RPC_URL\";
    collateral_registry = \"$COLLATERAL_REGISTRY\";
    strategies = vec {
      $strategies_vec
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
while true; do
    echo -n "> Do you have already deployed Batch Managers? (y/n): "
    read answer
    if [ "$answer" == "y" ]; then
        while true; do
            echo -n "> Enter the Batch Manager address: "
            read BATCH_MANAGER
            batch_managers+=("$BATCH_MANAGER")
            
            echo -n "> Enter the target_min value for this Batch Manager: "
            read TARGET_MIN
            target_mins+=("$TARGET_MIN")
            
            echo -n "> Do you have more Batch Managers? (y/n): "
            read more_managers
            if [ "$more_managers" == "n" ]; then
                break
            fi
        done
        break
    elif [ "$answer" == "n" ]; then
        while true; do
            echo -n "> Enter the target_min value for the new Batch Manager: "
            read TARGET_MIN
            target_mins+=("$TARGET_MIN")
            
            echo -n "> Do you want to add more target_min values? (y/n): "
            read more_values
            if [ "$more_values" == "n" ]; then
                break
            fi
        done
        break
    else
        echo -e "> ${ERROR_COLOR}Invalid input. Please enter 'y' or 'n'.${RESET_COLOR}"
    fi
done

run_script

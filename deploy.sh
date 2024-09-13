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

strategies=()
batch_managers=()
eoa_addresses=()

# Load environment variables from the .env file
echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
echo -e "> [INFO] Loading environment variables from .env file...${RESET_COLOR}"
export $(grep -v '^#' .env | xargs)
if [ $? -ne 0 ]; then
    echo -e "> ${ERROR_COLOR}[ERROR] Failed to load environment variables!${RESET_COLOR}"
    exit 1
fi
echo -e "> ${SUCCESS_COLOR}[INFO] Environment variables loaded successfully.${RESET_COLOR}"
echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"

# Function to ask for strategy data
ask_strategy_data() {
    echo "> Please enter the following details for the strategy:"
    read -p "Key: " key
    read -p "Manager: " manager
    read -p "Hint Helper: " hint_helper
    read -p "Collateral Index: " collateral_index
    read -p "Multi Trove Getter: " multi_trove_getter
    read -p "Upfront Fee Period: " upfront_fee_period
    read -p "RPC URL: " rpc_url
    read -p "Target Min: " target_min
    read -p "Collateral Registry: " collateral_registry

    strategy="record { key=$key; manager=\"$manager\"; rpc_principal=principal \"$RPC_PRINCIPAL\"; hint_helper=\"$hint_helper\"; collateral_index=$collateral_index; multi_trove_getter=\"$multi_trove_getter\"; upfront_fee_period=$upfront_fee_period; rpc_url=\"$rpc_url\"; target_min=$target_min; collateral_registry=\"$collateral_registry\" }"
    strategies+=("$strategy")
}

# Function to run the script logic
run_script() {
    # Step 1: Deploy the canister to the Internet Computer (IC)
    echo -e "> ${INFO_COLOR}[INFO] Deploying the canister to the Internet Computer (IC)...${RESET_COLOR}"
    deploy_output=$(dfx deploy --ic)
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Canister deployment failed!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] Canister deployed successfully.${RESET_COLOR}"
    echo -e "> ${INFO_COLOR}${BREAK_LINE}${RESET_COLOR}"
    
    # Step 2: Mint strategies and capture EOA addresses
    for strategy in "${strategies[@]}"; do
        echo -e "> [INFO] Minting strategy: $strategy${RESET_COLOR}"
        mint_output=$(dfx canister call --ic ir_manager mint_strategy "($strategy)")
        if [ $? -ne 0 ]; then
            echo -e "> ${ERROR_COLOR}[ERROR] Failed to mint strategy!${RESET_COLOR}"
            exit 1
        fi

        # Extract the EOA address from the mint output
        EOA=$(echo "$mint_output" | sed -n 's/.*variant { Ok = "\(0x[0-9a-fA-F]\{40\}\)" }.*/\1/p')
        eoa_addresses+=("$EOA")
        echo -e "> ${SUCCESS_COLOR}[INFO] Strategy minted successfully. EOA: $EOA${RESET_COLOR}"
    done
    
    # Step 3: Deploy batch managers for each strategy
    echo -e "> ${INFO_COLOR}[INFO] Deploying batch managers using Forge...${RESET_COLOR}"
    cd contracts
    for (( i=0; i<${#strategies[@]}; i++ )); do
        forge_output=$(forge create --rpc-url "$RPC_URL" \
            --constructor-args "${eoa_addresses[i]}" "$MANAGER" "$BORROWER_OPERATIONS" "$BOLD_TOKEN" "$WETH_PRICE_FEED" "$SORTED_TROVES" "$MIN_INTEREST_RATE" "$MAX_INTEREST_RATE" "$CURRENT_RATE" "$ANNUAL_MANAGEMENT_FEE" "$MIN_INTEREST_RATE_CHANGE_PERIOD" "$DISCOUNT_RATE_BATCH_MANAGER" \
            --private-key "$PRIVATE_KEY"\
            src/BatchManager.sol:BatchManager)
        BATCH_MANAGER=$(echo "$forge_output" | grep -oP 'Deployed to: (0x[0-9a-fA-F]{40})' | sed 's/Deployed to: //' | head -n 1)
        batch_managers+=("$BATCH_MANAGER")
    done
    cd ..

    # Step 4: Set batch manager IDs for strategies
    for (( i=0; i<${#strategies[@]}; i++ )); do
        echo -e "> [INFO] Setting batch manager ID for strategy $i...${RESET_COLOR}"
        dfx canister call --ic ir_manager set_batch_manager "($i, \"${batch_managers[i]}\")"
        if [ $? -ne 0 ]; then
            echo -e "> ${ERROR_COLOR}[ERROR] Failed to set batch manager ID!${RESET_COLOR}"
            exit 1
        fi
    done

    # Step 5: Start timers on the canister
    echo -e "> [INFO] Starting timers on the canister...${RESET_COLOR}"
    dfx canister --ic call ir_manager start_timers
    if [ $? -ne 0 ]; then
        echo -e "> ${ERROR_COLOR}[ERROR] Failed to start timers!${RESET_COLOR}"
        exit 1
    fi
    echo -e "> ${SUCCESS_COLOR}[INFO] Timers started successfully.${RESET_COLOR}"
}

# Collect strategy data
while true; do
    ask_strategy_data
    echo -n "> Do you want to add one more strategy? (y/n): "
    read more_strategies
    if [ "$more_strategies" == "n" ]; then
        break
    fi
done

# Ask if the user wants to create mock troves
echo -n "> Do you want me to create mock troves? (y/n): "
read create_mock_troves

if [ "$create_mock_troves" == "y" ]; then
    # Logic to create mock troves
    echo -e "> ${INFO_COLOR}[INFO] Creating mock troves...${RESET_COLOR}"
fi

# Run the main deployment script
run_script

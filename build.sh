#!/bin/bash

# Colors and formatting
BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Log function with timestamp and fancy formatting
log() {
    local level=$1
    local message=$2
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    case $level in
        "INFO")
            echo -e "${CYAN}[${timestamp}]${NC} ${message}"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[${timestamp}]${NC} ✨ ${BOLD}${message}${NC} ✨"
            ;;
        "ERROR")
            echo -e "${RED}[${timestamp}]${NC} ❌ ${BOLD}${message}${NC}"
            ;;
    esac
}

# Draw header
echo
echo -e "${MAGENTA}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${MAGENTA}║${NC}                ${BOLD}IR Manager Build System${NC}                ${MAGENTA}║${NC}"
echo -e "${MAGENTA}╚══════════════════════════════════════════════════════════╝${NC}"
echo

# Run fix and format script
log "INFO" "Running fix and format script..."
sh fix_and_fmt.sh

# Generate candid interface
log "INFO" "Generating Candid interface..."
cd ir_manager
if cargo run --features export-api > candid.did; then
    log "SUCCESS" "Candid interface generated successfully"
else
    log "ERROR" "Failed to generate Candid interface"
    exit 1
fi

# Build WASM
log "INFO" "Building WASM binary..."
cd ..
if cargo build --release --target wasm32-unknown-unknown --features export-api; then
    log "SUCCESS" "WASM binary built successfully"
else
    log "ERROR" "Failed to build WASM binary"
    exit 1
fi

# Footer
echo
echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}      ${BOLD}Build Complete${NC}                    ${BLUE}║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo
#!/bin/bash

# Script to build and format the IR Manager canister
# This script:
# 1. Generates Candid interface
# 2. Builds optimized WASM binary
# 3. Performs error checking at each step

# Exit on any error
set -e

# Enable command printing for debugging
set -x

# Constants
CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Project directories
IR_MANAGER_DIR="ir_manager"
TARGET_DIR="target/wasm32-unknown-unknown/release"

# Log function for consistent formatting
log() {
    echo -e "${CYAN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}" >&2
    exit 1
}

success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

# Check if required tools are installed
check_dependencies() {
    log "Checking dependencies..."
    
    if ! command -v cargo &> /dev/null; then
        error "cargo is not installed. Please install Rust toolchain."
    fi

    if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
        log "Installing WASM target..."
        rustup target add wasm32-unknown-unknown || error "Failed to install WASM target"
    }
}

# Generate Candid interface
generate_candid() {
    log "Generating Candid interface..."
    
    cd "$IR_MANAGER_DIR" || error "Failed to enter ir_manager directory"
    
    if ! cargo run --features export-api > candid.did; then
        error "Failed to generate Candid interface"
    fi
    
    if [ ! -s candid.did ]; then
        error "Generated Candid file is empty"
    fi
    
    success "Generated Candid interface"
    cd ..
}

# Build optimized WASM binary
build_wasm() {
    log "Building optimized WASM binary..."
    
    RUSTFLAGS='-C link-arg=-s' cargo build \
        --release \
        --target wasm32-unknown-unknown \
        --features export-api || error "WASM build failed"

    # Verify the WASM file exists and has non-zero size
    if [ ! -s "$TARGET_DIR/ir_manager.wasm" ]; then
        error "WASM binary not generated or empty"
    fi
    
    success "Built WASM binary successfully"
    
    # Print binary size for information
    WASM_SIZE=$(ls -lh "$TARGET_DIR/ir_manager.wasm" | awk '{print $5}')
    log "WASM binary size: $WASM_SIZE"
}

# Main execution
main() {
    log "Starting build process..."
    
    check_dependencies
    generate_candid
    build_wasm
    
    success "Build completed successfully!"
}

# Execute main function
main

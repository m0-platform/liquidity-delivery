#!/bin/bash
set -e

# Deterministic addresses from private keys
SOLVER_ADDRESS="0xd39f64F38761c2B1a1056dAbc10ABCf3ef33133C"
USER_ADDRESS="0xc5b879eB5dfe67dC612Ca6971cA4DFcFB8915adF"

# Anvil's default funded account (account 0)
ANVIL_ACCOUNT="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
ANVIL_PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# Chain configurations
CHAIN_A_RPC="${CHAIN_A_RPC:-http://anvil-chain-a:8545}"
CHAIN_B_RPC="${CHAIN_B_RPC:-http://anvil-chain-b:8545}"
CHAIN_A_ID=1
CHAIN_B_ID=8453
SOLANA_CHAIN_ID=1399811149

# Function to wait for RPC to be ready
wait_for_rpc() {
    local rpc_url=$1
    local name=$2
    local max_attempts=30
    local attempt=1

    echo "Waiting for $name to be ready..."
    while [ $attempt -le $max_attempts ]; do
        if curl -s -X POST -H "Content-Type: application/json" \
            --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
            "$rpc_url" > /dev/null 2>&1; then
            echo "$name is ready!"
            return 0
        fi
        echo "  Attempt $attempt/$max_attempts - waiting..."
        sleep 1
        attempt=$((attempt + 1))
    done
    echo "ERROR: $name failed to become ready"
    exit 1
}

# Function to fund an account
fund_account() {
    local rpc_url=$1
    local to_address=$2
    local amount=$3  # in wei
    local name=$4

    echo "Funding $name ($to_address) with 10 ETH..."
    cast send --rpc-url "$rpc_url" \
        --private-key "$ANVIL_PRIVATE_KEY" \
        "$to_address" \
        --value "$amount" \
        > /dev/null 2>&1
    echo "  Funded successfully"
}

# Function to deploy contracts to a chain
deploy_to_chain() {
    local rpc_url=$1
    local chain_id=$2
    local dest_chain_id=$3
    local chain_name=$4

    echo ""
    echo "Deploying to $chain_name (Chain ID: $chain_id)..."
    echo "----------------------------------------"

    cd /app/evm

    # Run the deployment script
    # DEST_CHAIN_IDS is a comma-separated list of destination chain IDs
    CHAIN_ID=$chain_id \
    DEST_CHAIN_IDS="$dest_chain_id,$SOLANA_CHAIN_ID" \
    SOLVER_ADDRESS=$SOLVER_ADDRESS \
    USER_ADDRESS=$USER_ADDRESS \
    forge script script/deploy/DeployLocal.s.sol:DeployLocal \
        --rpc-url "$rpc_url" \
        --broadcast \
        --skip-simulation \
        2>&1 | tee /tmp/deploy_${chain_id}.log
}

# Wait for both Anvil nodes
wait_for_rpc "$CHAIN_A_RPC" "Chain A (Anvil)"
wait_for_rpc "$CHAIN_B_RPC" "Chain B (Anvil)"

echo "Both chains are ready. Funding accounts..."

# Fund solver and user accounts on both chains (10 ETH each)
FUNDING_AMOUNT="10000000000000000000"  # 10 ETH in wei

fund_account "$CHAIN_A_RPC" "$SOLVER_ADDRESS" "$FUNDING_AMOUNT" "Solver (Chain A)"
fund_account "$CHAIN_A_RPC" "$USER_ADDRESS" "$FUNDING_AMOUNT" "User (Chain A)"
fund_account "$CHAIN_B_RPC" "$SOLVER_ADDRESS" "$FUNDING_AMOUNT" "Solver (Chain B)"
fund_account "$CHAIN_B_RPC" "$USER_ADDRESS" "$FUNDING_AMOUNT" "User (Chain B)"

echo "Deploying contracts..."

# Deploy to Chain A (destination is Chain B)
deploy_to_chain "$CHAIN_A_RPC" "$CHAIN_A_ID" "$CHAIN_B_ID" "Chain A"

# Deploy to Chain B (destination is Chain A)
deploy_to_chain "$CHAIN_B_RPC" "$CHAIN_B_ID" "$CHAIN_A_ID" "Chain B"

echo "Deployment Complete!"


#!/bin/bash
set -e

# Solana accounts to fund
USER_1="24PNhTaNtomHhoy3fTRaMhAFCRj4uHqhZEEoWrKDbR5p"
USER_2="test4MzZzYk2NAP1222FSuKqq83GuXY5tHakqREDHPo"

USDC_MINT="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
SVM_EXT="usdsfJbX78ktZUnoRC7dwvvQz7xH3WdkpGne76gdUia"
AMOUNT=1000000000

# Surfpool RPC endpoint
SURFPOOL_RPC="${SURFPOOL_RPC:-http://surfpool:8899}"


# Function to wait for Surfpool RPC to be ready
wait_for_surfpool() {
    local max_attempts=30
    local attempt=1

    echo "Waiting for Surfpool to be ready..."
    while [ $attempt -le $max_attempts ]; do
        if curl -s -X POST -H "Content-Type: application/json" \
            --data '{"jsonrpc":"2.0","method":"getHealth","params":[],"id":1}' \
            "$SURFPOOL_RPC" 2>/dev/null | grep -q "ok"; then
            echo "Surfpool is ready!"
            return 0
        fi
        echo "  Attempt $attempt/$max_attempts - waiting..."
        sleep 2
        attempt=$((attempt + 1))
    done
    echo "ERROR: Surfpool failed to become ready"
    exit 1
}

# Function to fund a Solana account with tokens using Surfpool cheatcode
fund_token_account() {
    local owner=$1
    local mint=$2
    local amount=$3
    local name=$4

    echo "Funding $name ($owner) with tokens..."

    response=$(curl -s -X POST "$SURFPOOL_RPC" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"id\": 1,
            \"method\": \"surfnet_setTokenAccount\",
            \"params\": [
                \"$owner\",
                \"$mint\",
                {\"amount\": $amount}
            ]
        }")

    if echo "$response" | grep -q '"error"'; then
        echo "  ERROR: Failed to fund $name"
        echo "  Response: $response"
        return 1
    fi

    echo "  Funded successfully"
}

# Function to verify token balance
verify_balance() {
    local owner=$1
    local mint=$2
    local name=$3

    balance=$(curl -s -X POST "$SURFPOOL_RPC" \
        -H "Content-Type: application/json" \
        -d "{
            \"jsonrpc\": \"2.0\",
            \"id\": 1,
            \"method\": \"getTokenAccountsByOwner\",
            \"params\": [
                \"$owner\",
                {\"mint\": \"$mint\"},
                {\"encoding\": \"jsonParsed\"}
            ]
        }" | grep -o '"uiAmountString":"[^"]*"' | head -1 | cut -d'"' -f4)

    echo "  $name balance: $balance USDC"
}

wait_for_surfpool

echo "Funding accounts with USDC..."
fund_token_account "$USER_1" "$USDC_MINT" "$AMOUNT" "User 1"
fund_token_account "$USER_2" "$USDC_MINT" "$AMOUNT" "User 2"
fund_token_account "$USER_1" "$SVM_EXT" "$AMOUNT" "User 1"
fund_token_account "$USER_2" "$SVM_EXT" "$AMOUNT" "User 2"

echo "Verifying balances..."
verify_balance "$USER_1" "$USDC_MINT" "User 1"
verify_balance "$USER_2" "$USDC_MINT" "User 2"
verify_balance "$USER_1" "$SVM_EXT" "User 1"
verify_balance "$USER_2" "$SVM_EXT" "User 2"

echo "Accounts funded:"
echo "  User 1: $USER_1"
echo "  User 2: $USER_2"
echo ""

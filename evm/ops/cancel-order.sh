#!/bin/bash
# Cancel an order on the OrderBook
# Uses Forge multichain forks to query OrderData from the origin chain
# Supports default (Hyperlane), Wormhole, and custom bridge adapters
# Usage: ./ops/cancel-order.sh --env dev --chain arbitrum_sepolia \
#            --order-id 0x... --origin-chain sepolia
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVM_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE=""  # Set after env is parsed: chains.dev.json or chains.prod.json

# 1Password account
OP_ACCOUNT="mzerolabs.1password.com"

# Wormhole executor API
WORMHOLE_EXECUTOR_API="https://executor.labsapis.com/v0/quote"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# EVM chain ID → Wormhole chain ID mapping
get_wormhole_chain_id() {
    local evm_chain_id=$1
    case "$evm_chain_id" in
        1)         echo 2 ;;      # Ethereum Mainnet
        8453)      echo 30 ;;     # Base
        2288)      echo 63 ;;     # Moca
        11155111)  echo 10002 ;;  # Sepolia
        421614)    echo 10003 ;;  # Arbitrum Sepolia
        84532)     echo 10004 ;;  # Base Sepolia
        *)
            log_error "No Wormhole chain ID mapping for EVM chain ID: $evm_chain_id"
            exit 1
            ;;
    esac
}

# Check dependencies
check_dependencies() {
    if ! command -v jq &> /dev/null; then
        log_error "jq is required but not installed. Install with: brew install jq"
        exit 1
    fi
    if ! command -v forge &> /dev/null; then
        log_error "forge is required but not installed. See: https://getfoundry.sh"
        exit 1
    fi
    if ! command -v op &> /dev/null; then
        log_error "1Password CLI (op) is required but not installed. See: https://developer.1password.com/docs/cli"
        exit 1
    fi
}

# Get env file path
get_env_file() {
    local env=$1
    echo "$EVM_DIR/.env.$env"
}

# Validate environment
validate_env() {
    local env=$1
    local env_file=$(get_env_file "$env")

    if [[ ! -f "$env_file" ]]; then
        log_error "Environment file not found: $env_file"
        log_error "Valid environments: dev, prod"
        exit 1
    fi
}

# Get chain config by alias
get_chain_id() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].chainId // empty' "$CONFIG_FILE"
}

get_rpc_alias() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].rpcAlias // empty' "$CONFIG_FILE"
}

get_chain_name() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].name // empty' "$CONFIG_FILE"
}

get_portal_address() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].portal // empty' "$CONFIG_FILE"
}

get_adapter_address() {
    local chain_alias=$1
    local adapter_name=$2
    jq -r --arg a "$chain_alias" --arg b "$adapter_name" '.chains[$a].adapters[$b] // empty' "$CONFIG_FILE"
}

# Encode Wormhole relay instructions (Gas type = 0x01, uint128 gasLimit, uint128 msgValue)
encode_relay_instructions() {
    local gas_limit=$1
    local msg_value=${2:-0}
    # Type byte (01) + uint128 gasLimit (32 hex chars) + uint128 msgValue (32 hex chars)
    printf "0x01%032x%032x" "$gas_limit" "$msg_value"
}

# Fetch Wormhole quote from executor API
fetch_wormhole_quote() {
    local src_wormhole_id=$1
    local dst_wormhole_id=$2
    local relay_instructions=$3

    log_info "Fetching Wormhole quote..."
    log_info "  Source (Wormhole ID): $src_wormhole_id"
    log_info "  Destination (Wormhole ID): $dst_wormhole_id"
    log_info "  Relay Instructions: $relay_instructions"

    local response
    response=$(curl -s -X POST "$WORMHOLE_EXECUTOR_API" \
        -H "Content-Type: application/json" \
        -d "{\"srcChain\":$src_wormhole_id,\"dstChain\":$dst_wormhole_id,\"relayInstructions\":\"$relay_instructions\"}")

    local signed_quote
    local estimated_cost
    signed_quote=$(echo "$response" | jq -r '.signedQuote // empty')
    estimated_cost=$(echo "$response" | jq -r '.estimatedCost // empty')

    if [[ -z "$signed_quote" || -z "$estimated_cost" ]]; then
        log_error "Failed to get Wormhole quote. Response: $response"
        exit 1
    fi

    log_info "  Signed Quote: ${signed_quote:0:40}..."
    log_info "  Estimated Cost: $estimated_cost"

    # Return values via global vars
    WORMHOLE_SIGNED_QUOTE="$signed_quote"
    WORMHOLE_ESTIMATED_COST="$estimated_cost"
}

# Show usage
usage() {
    echo "Cancel an order on the OrderBook"
    echo ""
    echo "Uses Forge multichain forks to query OrderData from the origin chain."
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias> \\"
    echo "     --order-id <bytes32> --origin-chain <alias> \\"
    echo "     [--bridge-adapter <address>] [--bridge-adapter-args <bytes>] [--wormhole]"
    echo ""
    echo "Required arguments:"
    echo "  --env                 Environment (dev or prod)"
    echo "  --chain               Chain alias where to cancel (e.g., arbitrum_sepolia)"
    echo "  --order-id            Order ID (bytes32)"
    echo "  --origin-chain        Chain alias where order was created (e.g., sepolia)"
    echo ""
    echo "Optional arguments:"
    echo "  --bridge-adapter      Bridge adapter address for cross-chain messaging"
    echo "  --bridge-adapter-args Bridge adapter args (e.g., signed Wormhole quote)"
    echo "  --wormhole            Use Wormhole adapter (auto-fetches quote from executor API)"
    echo "  --gas-limit           Override payload gas limit (skips Portal query)"
    echo ""
    echo "Environment variables:"
    echo "  DRY_RUN=true          Simulate without broadcasting"
    echo ""
    echo "Authorization rules:"
    echo "  - Before deadline: recipient (or sender for same-chain) can cancel"
    echo "  - After deadline: anyone can cancel (permissionless refund)"
    echo ""
    echo "Examples:"
    echo "  # Cross-chain cancel with default adapter (Hyperlane)"
    echo "  $0 --env dev --chain arbitrum_sepolia \\"
    echo "     --order-id 0x1234... --origin-chain sepolia"
    echo ""
    echo "  # Cancel with Wormhole adapter"
    echo "  $0 --env prod --chain moca \\"
    echo "     --order-id 0x1234... --origin-chain mainnet --wormhole"
    echo ""
    echo "  DRY_RUN=true $0 --env dev --chain arbitrum_sepolia ..."
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local order_id=""
    local origin_chain=""
    local bridge_adapter="0x0000000000000000000000000000000000000000"
    local bridge_adapter_args="0x"
    local use_wormhole=false
    local gas_limit_override=""

    while [[ $# -gt 0 ]]; do
        case $1 in
            --env|-e)
                env="$2"
                shift 2
                ;;
            --chain|-c)
                chain="$2"
                shift 2
                ;;
            --order-id)
                order_id="$2"
                shift 2
                ;;
            --origin-chain)
                origin_chain="$2"
                shift 2
                ;;
            --bridge-adapter)
                bridge_adapter="$2"
                shift 2
                ;;
            --bridge-adapter-args)
                bridge_adapter_args="$2"
                shift 2
                ;;
            --wormhole)
                use_wormhole=true
                shift
                ;;
            --gas-limit)
                gas_limit_override="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done

    # Validate required arguments
    if [[ -z "$env" ]]; then
        log_error "Environment is required. Use --env dev or --env prod"
        usage
        exit 1
    fi
    if [[ -z "$chain" ]]; then
        log_error "Chain is required. Use --chain <alias>"
        usage
        exit 1
    fi
    if [[ -z "$order_id" ]]; then
        log_error "Order ID is required. Use --order-id <bytes32>"
        usage
        exit 1
    fi
    if [[ -z "$origin_chain" ]]; then
        log_error "Origin chain is required. Use --origin-chain <alias>"
        usage
        exit 1
    fi

    validate_env "$env"
    CONFIG_FILE="$EVM_DIR/config/chains.${env}.json"

    # Validate chains exist in config
    local origin_chain_id=$(get_chain_id "$origin_chain")
    local origin_chain_name=$(get_chain_name "$origin_chain")
    if [[ -z "$origin_chain_id" ]]; then
        log_error "Origin chain alias '$origin_chain' not found in config"
        exit 1
    fi

    local env_file=$(get_env_file "$env")
    local chain_id=$(get_chain_id "$chain")
    local rpc_alias=$(get_rpc_alias "$chain")
    local chain_name=$(get_chain_name "$chain")

    if [[ -z "$chain_id" ]]; then
        log_error "Chain alias '$chain' not found in config"
        exit 1
    fi

    # Get portal address from config
    local portal_address=$(get_portal_address "$chain")
    if [[ -z "$portal_address" ]]; then
        log_error "Portal address not found in config for chain '$chain'"
        exit 1
    fi

    log_step "Cancelling order on $chain_name [env: $env]"
    log_info "Order ID: $order_id"
    log_info "Origin Chain: $origin_chain_name (ID: $origin_chain_id)"
    log_info "Portal: $portal_address"

    if [[ "$origin_chain_id" == "$chain_id" ]]; then
        log_info "Cancel Type: Same-chain (immediate refund)"
    else
        log_info "Cancel Type: Cross-chain (will send cancel report to origin)"
    fi
    log_info "OrderData will be queried from origin chain via fork"

    # Handle Wormhole adapter
    local bridge_fee=""
    if [[ "$use_wormhole" == true ]]; then
        local wormhole_adapter=$(get_adapter_address "$chain" "wormhole")
        if [[ -z "$wormhole_adapter" ]]; then
            log_error "Wormhole adapter address not found in config for chain '$chain'"
            exit 1
        fi
        bridge_adapter="$wormhole_adapter"
        log_info "Bridge Adapter: Wormhole ($wormhole_adapter)"

        # Get payload gas limit — use override if provided, otherwise query Portal
        local gas_limit
        if [[ -n "$gas_limit_override" ]]; then
            gas_limit="$gas_limit_override"
            log_info "Payload gas limit (override): $gas_limit"
        else
            log_info "Querying payload gas limit from Portal (PayloadType.CancelReport=6)..."
            local gas_limit_raw
            gas_limit_raw=$(op run --env-file="$env_file" --account="$OP_ACCOUNT" -- \
                cast call "$portal_address" "payloadGasLimit(uint32,uint8)(uint256)" \
                "$origin_chain_id" 6 --rpc-url "$rpc_alias" 2>&1) || true
            gas_limit=$(echo "$gas_limit_raw" | grep -oE '^[0-9]+' | head -1) || true
            log_info "Payload gas limit: ${gas_limit:-<empty>}"

            if [[ -z "$gas_limit" || "$gas_limit" == "0" ]]; then
                log_error "Failed to read payload gas limit from Portal on $chain_name for origin chain $origin_chain_id"
                log_error "Raw output: $gas_limit_raw"
                log_error "Use --gas-limit <value> to specify manually"
                exit 1
            fi
        fi

        # Encode relay instructions
        local relay_instructions
        relay_instructions=$(encode_relay_instructions "$gas_limit" 0)
        log_info "Relay instructions: $relay_instructions"

        # Map chain IDs to Wormhole IDs (source = dest chain where cancel happens, dest = origin chain where report goes)
        local src_wormhole_id=$(get_wormhole_chain_id "$chain_id")
        local dst_wormhole_id=$(get_wormhole_chain_id "$origin_chain_id")

        # Fetch Wormhole quote
        fetch_wormhole_quote "$src_wormhole_id" "$dst_wormhole_id" "$relay_instructions"
        bridge_adapter_args="$WORMHOLE_SIGNED_QUOTE"

        # Get core bridge fee
        log_info "Querying Wormhole core bridge fee..."
        local core_bridge_raw core_bridge
        core_bridge_raw=$(op run --env-file="$env_file" --account="$OP_ACCOUNT" -- \
            cast call "$wormhole_adapter" "coreBridge()(address)" --rpc-url "$rpc_alias" 2>&1) || true
        core_bridge=$(echo "$core_bridge_raw" | grep -oE '0x[0-9a-fA-F]+' | head -1) || true
        log_info "Core bridge address: ${core_bridge:-<empty>}"

        local core_bridge_fee=0
        if [[ -n "$core_bridge" ]]; then
            local core_bridge_fee_raw
            core_bridge_fee_raw=$(op run --env-file="$env_file" --account="$OP_ACCOUNT" -- \
                cast call "$core_bridge" "messageFee()(uint256)" --rpc-url "$rpc_alias" 2>&1) || true
            core_bridge_fee=$(echo "$core_bridge_fee_raw" | grep -oE '^[0-9]+' | head -1) || true
            core_bridge_fee=${core_bridge_fee:-0}
        fi
        log_info "Core bridge fee: $core_bridge_fee"

        # Total bridge fee = executor estimated cost + core bridge fee
        local executor_cost="${WORMHOLE_ESTIMATED_COST:-0}"
        log_info "Executor estimated cost: $executor_cost"
        bridge_fee=$(echo "$executor_cost + $core_bridge_fee" | bc)

        if [[ -z "$bridge_fee" || "$bridge_fee" == "0" ]]; then
            log_error "Bridge fee computation failed or is zero (executor_cost=$executor_cost, core_bridge_fee=$core_bridge_fee)"
            exit 1
        fi
        log_info "Total bridge fee (wei): $bridge_fee"
    elif [[ "$bridge_adapter" != "0x0000000000000000000000000000000000000000" ]]; then
        log_info "Bridge Adapter: $bridge_adapter"
    fi

    # Build forge command
    local forge_cmd="forge script script/test/CancelOrder.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    forge_cmd="$forge_cmd --ignored-error-codes 2424"
    forge_cmd="$forge_cmd --sig 'run(bytes32,string,address,bytes)'"
    forge_cmd="$forge_cmd $order_id '$origin_chain' $bridge_adapter $bridge_adapter_args"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating cancel (no broadcast)"
    else
        forge_cmd="$forge_cmd --broadcast"
    fi

    log_info "Running with 1Password: op run --env-file=$env_file --account=$OP_ACCOUNT"

    # Export env vars so they're visible to forge via op run
    local env_exports="export FOUNDRY_PROFILE=production PORTAL_ADDRESS=$portal_address"
    if [[ -n "$bridge_fee" ]]; then
        env_exports="$env_exports BRIDGE_FEE=$bridge_fee"
        log_info "BRIDGE_FEE=$bridge_fee"
    fi

    # Execute from EVM directory with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- bash -c "$env_exports && DRY_RUN=${DRY_RUN:-false} $forge_cmd"

    if [[ "${DRY_RUN:-false}" != "true" ]]; then
        log_info "Order cancel complete!"
    fi
}

main "$@"

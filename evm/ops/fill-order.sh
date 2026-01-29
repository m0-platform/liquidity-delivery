#!/bin/bash
# Fill a test order on the OrderBook
# Uses Forge multichain forks to query OrderData from the origin chain
# Usage: ./ops/fill-order.sh --env dev --chain arbitrum_sepolia \
#            --order-id 0x... --origin-chain sepolia --amount-out 500000
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVM_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="$EVM_DIR/config/chains.json"

# 1Password account
OP_ACCOUNT="mzerolabs.1password.com"

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

# Convert address to bytes32
address_to_bytes32() {
    local addr=$1
    # Remove 0x prefix, pad to 64 chars (32 bytes), add 0x prefix
    local clean_addr=${addr#0x}
    printf "0x%064s" "$clean_addr" | tr ' ' '0'
}

# Show usage
usage() {
    echo "Fill a test order on the OrderBook"
    echo ""
    echo "Uses Forge multichain forks to query OrderData from the origin chain."
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias> \\"
    echo "     --order-id <bytes32> --origin-chain <alias> --amount-out <amount> \\"
    echo "     [--origin-recipient <address>] [--bridge-adapter-args <bytes>]"
    echo ""
    echo "Required arguments:"
    echo "  --env                 Environment (dev or prod)"
    echo "  --chain               Chain alias where to fill (e.g., arbitrum_sepolia)"
    echo "  --order-id            Order ID (bytes32)"
    echo "  --origin-chain        Chain alias where order was created (e.g., sepolia)"
    echo "  --amount-out          Amount of output token to fill (supports partial)"
    echo ""
    echo "Optional arguments:"
    echo "  --origin-recipient    Address on origin chain to receive released funds"
    echo "                        (defaults to solver address)"
    echo "  --bridge-adapter-args Bridge adapter args (e.g., signed Wormhole quote)"
    echo ""
    echo "Environment variables:"
    echo "  DRY_RUN=true          Simulate without broadcasting"
    echo ""
    echo "Examples:"
    echo "  # Full fill (cross-chain)"
    echo "  $0 --env dev --chain arbitrum_sepolia \\"
    echo "     --order-id 0x1234... --origin-chain sepolia \\"
    echo "     --amount-out 1000000000000000000"
    echo ""
    echo "  # Partial fill (50%)"
    echo "  $0 --env dev --chain arbitrum_sepolia \\"
    echo "     --order-id 0x1234... --origin-chain sepolia \\"
    echo "     --amount-out 500000000000000000"
    echo ""
    echo "  # Same-chain fill"
    echo "  $0 --env dev --chain sepolia \\"
    echo "     --order-id 0x1234... --origin-chain sepolia \\"
    echo "     --amount-out 1000000000000000000"
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
    local amount_out=""
    local origin_recipient="0x0000000000000000000000000000000000000000000000000000000000000000"
    local bridge_adapter_args="0x"

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
            --amount-out)
                amount_out="$2"
                shift 2
                ;;
            --origin-recipient)
                origin_recipient=$(address_to_bytes32 "$2")
                shift 2
                ;;
            --bridge-adapter-args)
                bridge_adapter_args="$2"
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
    if [[ -z "$amount_out" ]]; then
        log_error "Amount out is required. Use --amount-out <amount>"
        usage
        exit 1
    fi

    # Validate origin chain exists in config
    local origin_chain_id=$(get_chain_id "$origin_chain")
    local origin_chain_name=$(get_chain_name "$origin_chain")
    if [[ -z "$origin_chain_id" ]]; then
        log_error "Origin chain alias '$origin_chain' not found in config"
        exit 1
    fi

    validate_env "$env"

    local env_file=$(get_env_file "$env")
    local chain_id=$(get_chain_id "$chain")
    local rpc_alias=$(get_rpc_alias "$chain")
    local chain_name=$(get_chain_name "$chain")

    if [[ -z "$chain_id" ]]; then
        log_error "Chain alias '$chain' not found in config"
        exit 1
    fi

    log_step "Filling order on $chain_name [env: $env]"
    log_info "Order ID: $order_id"
    log_info "Origin Chain: $origin_chain_name (ID: $origin_chain_id)"
    log_info "Amount Out to Fill: $amount_out"

    if [[ "$origin_chain_id" == "$chain_id" ]]; then
        log_info "Fill Type: Same-chain (immediate settlement)"
    else
        log_info "Fill Type: Cross-chain (will send fill report to origin)"
    fi
    log_info "OrderData will be queried from origin chain via fork"

    # Build forge command
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/test/FillOrder.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    forge_cmd="$forge_cmd --ignored-error-codes 2424"
    forge_cmd="$forge_cmd --sig 'run(bytes32,string,uint128,bytes32,bytes)'"
    forge_cmd="$forge_cmd $order_id '$origin_chain' $amount_out $origin_recipient $bridge_adapter_args"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating fill (no broadcast)"
    else
        forge_cmd="$forge_cmd --broadcast"
    fi

    log_info "Running with 1Password: op run --env-file=$env_file --account=$OP_ACCOUNT"

    # Execute from EVM directory with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- bash -c "DRY_RUN=${DRY_RUN:-false} $forge_cmd"

    if [[ "${DRY_RUN:-false}" != "true" ]]; then
        log_info "Order fill complete!"
    fi
}

main "$@"

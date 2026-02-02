#!/bin/bash
# Create a test order on the OrderBook
# Usage: ./ops/open-order.sh --env dev --chain sepolia \
#            --token-in 0x... --amount-in 1000000 \
#            --dest-chain arbitrum_sepolia --token-out 0x... --amount-out 1000000
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
    echo "Create a test order on the OrderBook"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias> \\"
    echo "     --token-in <address> --amount-in <amount> \\"
    echo "     --dest-chain <alias> --token-out <address> --amount-out <amount> \\"
    echo "     [--recipient <address>] [--solver <address>] [--deadline <seconds>]"
    echo ""
    echo "Required arguments:"
    echo "  --env             Environment (dev or prod)"
    echo "  --chain           Source chain alias (e.g., sepolia)"
    echo "  --token-in        Input token address on source chain"
    echo "  --amount-in       Amount of input token (in wei/smallest unit)"
    echo "  --dest-chain      Destination chain alias (e.g., arbitrum_sepolia)"
    echo "  --token-out       Output token address on destination chain"
    echo "  --amount-out      Amount of output token expected (in wei/smallest unit)"
    echo ""
    echo "Optional arguments:"
    echo "  --recipient       Recipient address on destination (defaults to sender)"
    echo "  --solver          Designated solver address (zero = any solver)"
    echo "  --deadline        Deadline offset in seconds (default: 3600 = 1 hour)"
    echo ""
    echo "Environment variables:"
    echo "  DRY_RUN=true      Simulate without broadcasting"
    echo ""
    echo "Examples:"
    echo "  $0 --env dev --chain sepolia \\"
    echo "     --token-in 0x1234... --amount-in 1000000000000000000 \\"
    echo "     --dest-chain arbitrum_sepolia --token-out 0x5678... --amount-out 1000000000000000000"
    echo ""
    echo "  DRY_RUN=true $0 --env dev --chain sepolia ..."
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local token_in=""
    local amount_in=""
    local dest_chain=""
    local token_out=""
    local amount_out=""
    local recipient="0x0000000000000000000000000000000000000000000000000000000000000000"
    local solver="0x0000000000000000000000000000000000000000000000000000000000000000"
    local deadline="0"

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
            --token-in)
                token_in="$2"
                shift 2
                ;;
            --amount-in)
                amount_in="$2"
                shift 2
                ;;
            --dest-chain)
                dest_chain="$2"
                shift 2
                ;;
            --token-out)
                token_out="$2"
                shift 2
                ;;
            --amount-out)
                amount_out="$2"
                shift 2
                ;;
            --recipient)
                recipient=$(address_to_bytes32 "$2")
                shift 2
                ;;
            --solver)
                solver=$(address_to_bytes32 "$2")
                shift 2
                ;;
            --deadline)
                deadline="$2"
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
    if [[ -z "$token_in" ]]; then
        log_error "Token in is required. Use --token-in <address>"
        usage
        exit 1
    fi
    if [[ -z "$amount_in" ]]; then
        log_error "Amount in is required. Use --amount-in <amount>"
        usage
        exit 1
    fi
    if [[ -z "$dest_chain" ]]; then
        log_error "Destination chain is required. Use --dest-chain <alias>"
        usage
        exit 1
    fi
    if [[ -z "$token_out" ]]; then
        log_error "Token out is required. Use --token-out <address>"
        usage
        exit 1
    fi
    if [[ -z "$amount_out" ]]; then
        log_error "Amount out is required. Use --amount-out <amount>"
        usage
        exit 1
    fi

    validate_env "$env"

    local env_file=$(get_env_file "$env")
    local chain_id=$(get_chain_id "$chain")
    local rpc_alias=$(get_rpc_alias "$chain")
    local chain_name=$(get_chain_name "$chain")
    local dest_chain_id=$(get_chain_id "$dest_chain")
    local dest_chain_name=$(get_chain_name "$dest_chain")

    if [[ -z "$chain_id" ]]; then
        log_error "Chain alias '$chain' not found in config"
        exit 1
    fi
    if [[ -z "$dest_chain_id" ]]; then
        log_error "Destination chain alias '$dest_chain' not found in config"
        exit 1
    fi

    # Convert token_out address to bytes32
    local token_out_bytes32=$(address_to_bytes32 "$token_out")

    log_step "Opening order on $chain_name -> $dest_chain_name [env: $env]"
    log_info "Token In: $token_in"
    log_info "Amount In: $amount_in"
    log_info "Destination Chain ID: $dest_chain_id"
    log_info "Token Out: $token_out"
    log_info "Amount Out: $amount_out"

    # Build forge command
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/test/OpenOrder.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    forge_cmd="$forge_cmd --ignored-error-codes 2424"
    forge_cmd="$forge_cmd --sig 'run(address,uint128,uint32,bytes32,uint128,bytes32,bytes32,uint32)'"
    forge_cmd="$forge_cmd $token_in $amount_in $dest_chain_id $token_out_bytes32 $amount_out $recipient $solver $deadline"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating order creation (no broadcast)"
    else
        forge_cmd="$forge_cmd --broadcast"
    fi

    log_info "Running with 1Password: op run --env-file=$env_file --account=$OP_ACCOUNT"

    # Execute from EVM directory with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- bash -c "DRY_RUN=${DRY_RUN:-false} $forge_cmd"

    if [[ "${DRY_RUN:-false}" != "true" ]]; then
        log_info "Order creation complete!"
        log_info "To fill this order, use the Order ID from the output above with:"
        log_info "  make fill-order ENV=$env CHAIN=<dest_chain> ORDER_ID=<order_id> ORIGIN_CHAIN=$chain AMOUNT_OUT=<amount>"
    fi
}

main "$@"

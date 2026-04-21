#!/bin/bash
# Transfer all privileged roles from deployer to a new admin on a single chain
# Usage: ./ops/transfer-roles.sh --env prod --chain base --new-admin 0x...
#        DRY_RUN=true ./ops/transfer-roles.sh --env prod --chain base --new-admin 0x...
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

# Get deployed OrderBook address for a chain
get_orderbook_address() {
    local chain_id=$1
    local deployment_file="$EVM_DIR/deployments/$chain_id.json"
    if [[ -f "$deployment_file" ]]; then
        jq -r '.orderBook // empty' "$deployment_file"
    fi
}

# Transfer roles on a single chain
transfer_roles() {
    local env=$1
    local alias=$2
    local new_admin=$3
    local env_file=$(get_env_file "$env")

    local chain_id=$(get_chain_id "$alias")
    local rpc_alias=$(get_rpc_alias "$alias")
    local chain_name=$(get_chain_name "$alias")

    if [[ -z "$chain_id" ]]; then
        log_error "Chain alias '$alias' not found in config"
        exit 1
    fi

    local orderbook=$(get_orderbook_address "$chain_id")
    if [[ -z "$orderbook" ]]; then
        log_error "No deployment found for $chain_name (chainId: $chain_id)"
        log_error "Run deploy first: ./ops/deploy.sh --env $env --chain $alias"
        exit 1
    fi

    # Validate new admin address format
    if [[ ! "$new_admin" =~ ^0x[0-9a-fA-F]{40}$ ]]; then
        log_error "Invalid new admin address: $new_admin"
        exit 1
    fi

    log_step "Transferring roles on $chain_name (chainId: $chain_id) [env: $env]"
    log_info "OrderBook proxy: $orderbook"
    log_info "New admin: $new_admin"

    # Build forge command
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/admin/TransferRoles.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd --sig 'run(address)' $new_admin"
    forge_cmd="$forge_cmd -vvv"
    # Ignore assembly NatSpec memory-safe deprecation warnings from forge-std
    forge_cmd="$forge_cmd --ignored-error-codes 2424"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating transfer (no broadcast)"
    else
        forge_cmd="$forge_cmd --broadcast"
    fi

    log_info "Running with 1Password: op run --env-file=$env_file --account=$OP_ACCOUNT"

    # Execute from EVM directory with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- bash -c "DRY_RUN=${DRY_RUN:-false} $forge_cmd"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Dry run complete for $chain_name"
    else
        log_info "Successfully transferred roles on $chain_name"
        log_info "New admin: $new_admin"
    fi
}

# Show usage
usage() {
    echo "Transfer all privileged roles from deployer to a new admin"
    echo ""
    echo "Transfers:"
    echo "  1. DEFAULT_ADMIN_ROLE on OrderBook"
    echo "  2. PAUSER_ROLE on OrderBook"
    echo "  3. ProxyAdmin ownership"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias> --new-admin <0x...>"
    echo ""
    echo "Options:"
    echo "  DRY_RUN=true  Simulate transfer without broadcasting"
    echo ""
    echo "Examples:"
    echo "  $0 --env prod --chain base --new-admin 0x1234...abcd"
    echo "  DRY_RUN=true $0 --env prod --chain mainnet --new-admin 0x1234...abcd"
    echo ""
    echo "Environment files:"
    echo "  .env.dev   - Development/testnet configuration"
    echo "  .env.prod  - Production/mainnet configuration"
    echo ""
    echo "Secrets are managed via 1Password CLI (op)."
    echo "Account: $OP_ACCOUNT"
    echo ""
    echo "Requires ADMIN_PRIVATE_KEY in the environment file."
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local new_admin=""

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
            --new-admin|-a)
                new_admin="$2"
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

    if [[ -z "$new_admin" ]]; then
        log_error "New admin address is required. Use --new-admin <0x...>"
        usage
        exit 1
    fi

    validate_env "$env"
    transfer_roles "$env" "$chain" "$new_admin"
}

main "$@"

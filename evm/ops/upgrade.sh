#!/bin/bash
# Upgrade OrderBook implementation on one or all chains
# Usage: ./ops/upgrade.sh --env dev --chain <alias>
#        ./ops/upgrade.sh --env prod --all
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

get_explorer_alias() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].explorerAlias // empty' "$CONFIG_FILE"
}

get_chain_name() {
    local alias=$1
    jq -r --arg a "$alias" '.chains[$a].name // empty' "$CONFIG_FILE"
}

get_all_chain_aliases() {
    jq -r '.chains | keys[]' "$CONFIG_FILE"
}

# Get deployed OrderBook address for a chain
get_orderbook_address() {
    local chain_id=$1
    local deployment_file="$EVM_DIR/deployments/$chain_id.json"
    if [[ -f "$deployment_file" ]]; then
        jq -r '.orderBook // empty' "$deployment_file"
    fi
}

# Upgrade on a single chain
upgrade_chain() {
    local env=$1
    local alias=$2
    local verify=${3:-false}
    local env_file=$(get_env_file "$env")

    local chain_id=$(get_chain_id "$alias")
    local rpc_alias=$(get_rpc_alias "$alias")
    local explorer_alias=$(get_explorer_alias "$alias")
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

    log_step "Upgrading OrderBook on $chain_name (chainId: $chain_id) [env: $env]"
    log_info "Proxy address: $orderbook"

    # Build forge command
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/deploy/Upgrade.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    # Ignore assembly NatSpec memory-safe deprecation warnings from forge-std
    forge_cmd="$forge_cmd --ignored-error-codes 2424"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating upgrade (no broadcast)"
    else
        forge_cmd="$forge_cmd --broadcast"
    fi

    if [[ "$verify" == "true" ]]; then
        forge_cmd="$forge_cmd --verify"
    fi

    log_info "Running with 1Password: op run --env-file=$env_file --account=$OP_ACCOUNT"

    # Execute from EVM directory with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- bash -c "DRY_RUN=${DRY_RUN:-false} $forge_cmd"

    # Show updated deployment info (skip file check in dry-run mode)
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Dry run complete for $chain_name"
    else
        local deployment_file="$EVM_DIR/deployments/$chain_id.json"
        if [[ -f "$deployment_file" ]]; then
            local new_impl=$(jq -r '.implementation // "unknown"' "$deployment_file")
            log_info "Successfully upgraded OrderBook on $chain_name"
            log_info "New implementation: $new_impl"
        fi
    fi
}

# Show current implementation status
show_status() {
    log_info "Current deployment status:"
    echo ""

    for alias in $(get_all_chain_aliases); do
        local chain_id=$(get_chain_id "$alias")
        local chain_name=$(get_chain_name "$alias")
        local deployment_file="$EVM_DIR/deployments/$chain_id.json"

        if [[ -f "$deployment_file" ]]; then
            local proxy=$(jq -r '.orderBook // "N/A"' "$deployment_file")
            local impl=$(jq -r '.implementation // "N/A"' "$deployment_file")
            local upgraded_at=$(jq -r '.upgradedAt // "N/A"' "$deployment_file")

            echo "$chain_name (chainId: $chain_id):"
            echo "  Proxy:          $proxy"
            echo "  Implementation: $impl"
            if [[ "$upgraded_at" != "N/A" && "$upgraded_at" != "null" ]]; then
                echo "  Last upgraded:  $(date -r "$upgraded_at" 2>/dev/null || echo "$upgraded_at")"
            fi
        else
            echo "$chain_name (chainId: $chain_id): NOT DEPLOYED"
        fi
        echo ""
    done
}

# Show usage
usage() {
    echo "Upgrade OrderBook implementation on configured chains"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias>           Upgrade on a specific chain"
    echo "  $0 --env <dev|prod> --chain <alias> --verify  Upgrade and verify on explorer"
    echo "  $0 --env <dev|prod> --all                     Upgrade on all deployed chains"
    echo "  $0 --status                                   Show current deployment status"
    echo ""
    echo "Options:"
    echo "  DRY_RUN=true  Simulate upgrade without broadcasting (logs JSON to console)"
    echo ""
    echo "Examples:"
    echo "  $0 --env dev --chain sepolia"
    echo "  $0 --env dev --chain arbitrum_sepolia --verify"
    echo "  $0 --env prod --all"
    echo "  DRY_RUN=true $0 --env dev --chain sepolia    # Dry run"
    echo ""
    echo "Environment files:"
    echo "  .env.dev   - Development/testnet configuration"
    echo "  .env.prod  - Production/mainnet configuration"
    echo ""
    echo "Secrets are managed via 1Password CLI (op)."
    echo "Account: $OP_ACCOUNT"
    echo ""
    echo "Note: The PRIVATE_KEY in the env file must be the owner of the ProxyAdmin,"
    echo "      which is typically the ADMIN_ADDRESS used during deployment."
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local all=false
    local verify=false
    local status=false

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
            --all|-a)
                all=true
                shift
                ;;
            --verify|-v)
                verify=true
                shift
                ;;
            --status|-s)
                status=true
                shift
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

    if [[ "$status" == "true" ]]; then
        show_status
        exit 0
    fi

    # Validate environment is specified
    if [[ -z "$env" ]]; then
        log_error "Environment is required. Use --env dev or --env prod"
        usage
        exit 1
    fi

    validate_env "$env"

    if [[ "$all" == "true" ]]; then
        log_info "Upgrading all deployed chains [env: $env]..."
        local upgraded=0
        for alias in $(get_all_chain_aliases); do
            local chain_id=$(get_chain_id "$alias")
            if [[ -n "$(get_orderbook_address "$chain_id")" ]]; then
                upgrade_chain "$env" "$alias" "$verify"
                echo ""
                ((upgraded++))
            else
                log_warn "Skipping $alias - not deployed"
            fi
        done

        if [[ $upgraded -eq 0 ]]; then
            log_error "No chains have been deployed yet"
            exit 1
        fi
        log_info "Upgraded $upgraded chains"
    elif [[ -n "$chain" ]]; then
        upgrade_chain "$env" "$chain" "$verify"
    else
        log_error "Must specify --chain <alias>, --all, or --status"
        usage
        exit 1
    fi
}

main "$@"

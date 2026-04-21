#!/bin/bash
# Deploy OrderBook to one or all configured chains
# Usage: ./ops/deploy.sh --env dev --chain <alias>
#        ./ops/deploy.sh --env prod --all
#        ./ops/deploy.sh --env dev --chain <alias> --verify
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

# Deploy to a single chain
deploy_chain() {
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

    log_step "Deploying to $chain_name (chainId: $chain_id) [env: $env]"

    # Check if already deployed (skip in dry-run mode)
    local deployment_file="$EVM_DIR/deployments/$chain_id.json"
    if [[ "${DRY_RUN:-false}" != "true" && -f "$deployment_file" ]]; then
        local existing_addr=$(jq -r '.orderBook // empty' "$deployment_file")
        if [[ -n "$existing_addr" ]]; then
            log_warn "OrderBook already deployed at $existing_addr"
            log_warn "Skipping deployment. Use upgrade command to update implementation."
            return 0
        fi
    fi

    # Build forge command
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/deploy/Deploy.s.sol"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    # Ignore assembly NatSpec memory-safe deprecation warnings from forge-std
    forge_cmd="$forge_cmd --ignored-error-codes 2424"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating deployment (no broadcast)"
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

    # Verify deployment file was created (skip in dry-run mode)
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Dry run complete for $chain_name"
    elif [[ -f "$deployment_file" ]]; then
        local deployed_addr=$(jq -r '.orderBook' "$deployment_file")
        log_info "Successfully deployed OrderBook to $deployed_addr on $chain_name"
    else
        log_error "Deployment file not created. Check for errors above."
        exit 1
    fi
}

# Show usage
usage() {
    echo "Deploy OrderBook to configured chains using 1Password for secrets"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias>           Deploy to a specific chain"
    echo "  $0 --env <dev|prod> --chain <alias> --verify  Deploy and verify on explorer"
    echo "  $0 --env <dev|prod> --all                     Deploy to all configured chains"
    echo "  $0 --env <dev|prod> --all --verify            Deploy to all chains with verification"
    echo "  $0 --list                                     List all configured chains"
    echo ""
    echo "Options:"
    echo "  DRY_RUN=true  Simulate deployment without broadcasting (logs JSON to console)"
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
}

# List configured chains
list_chains() {
    echo "Configured chains:"
    echo ""
    for alias in $(get_all_chain_aliases); do
        local chain_id=$(get_chain_id "$alias")
        local chain_name=$(get_chain_name "$alias")
        local deployment_file="$EVM_DIR/deployments/$chain_id.json"
        local status="NOT DEPLOYED"
        if [[ -f "$deployment_file" ]]; then
            local addr=$(jq -r '.orderBook // empty' "$deployment_file")
            if [[ -n "$addr" ]]; then
                status="$addr"
            fi
        fi
        printf "  %-20s %-20s %s\n" "$alias" "(chainId: $chain_id)" "$status"
    done
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local all=false
    local verify=false
    local list=false

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
            --list|-l)
                list=true
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

    if [[ "$list" == "true" ]]; then
        list_chains
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
        log_info "Deploying to all configured chains [env: $env]..."
        for alias in $(get_all_chain_aliases); do
            deploy_chain "$env" "$alias" "$verify"
            echo ""
        done
        log_info "All deployments complete!"
    elif [[ -n "$chain" ]]; then
        deploy_chain "$env" "$chain" "$verify"
    else
        log_error "Must specify --chain <alias> or --all"
        usage
        exit 1
    fi
}

main "$@"

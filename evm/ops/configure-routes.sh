#!/bin/bash
# Configure bidirectional destination routes between deployed OrderBooks
# Usage: ./ops/configure-routes.sh --env dev
#        ./ops/configure-routes.sh --env dev --source <alias> --dest <alias>
#        ./ops/configure-routes.sh --env dev --verify
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
    if ! command -v cast &> /dev/null; then
        log_error "cast is required but not installed. See: https://getfoundry.sh"
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

# Check if a destination is already configured on-chain
is_destination_configured() {
    local env=$1
    local rpc_alias=$2
    local orderbook=$3
    local dest_chain_id=$4
    local env_file=$(get_env_file "$env")

    local result=$(op run --env-file="$env_file" --account="$OP_ACCOUNT" -- \
        cast call "$orderbook" "isDestinationSupported(uint32)(bool)" "$dest_chain_id" --rpc-url "$rpc_alias" 2>/dev/null || echo "false")
    [[ "$result" == "true" ]]
}

# Configure a single route
configure_route() {
    local env=$1
    local source_alias=$2
    local dest_alias=$3
    local env_file=$(get_env_file "$env")

    local source_chain_id=$(get_chain_id "$source_alias")
    local dest_chain_id=$(get_chain_id "$dest_alias")
    local source_rpc=$(get_rpc_alias "$source_alias")
    local source_name=$(get_chain_name "$source_alias")
    local dest_name=$(get_chain_name "$dest_alias")

    local orderbook=$(get_orderbook_address "$source_chain_id")

    if [[ -z "$orderbook" ]]; then
        log_warn "No deployment found for $source_name (chainId: $source_chain_id), skipping"
        return 0
    fi

    # Check if already configured
    if is_destination_configured "$env" "$source_rpc" "$orderbook" "$dest_chain_id"; then
        log_info "Route $source_name -> $dest_name already configured"
        return 0
    fi

    log_step "Configuring route: $source_name -> $dest_name [env: $env]"

    # Run the forge script with op run
    cd "$EVM_DIR"
    op run --env-file="$env_file" --account="$OP_ACCOUNT" -- \
        bash -c "FOUNDRY_PROFILE=production forge script script/config/ConfigureDestination.s.sol \
            --rpc-url $source_rpc \
            --broadcast \
            --sig 'run(address,uint32,bool)' \
            $orderbook $dest_chain_id true \
            -vvv"

    log_info "Route $source_name -> $dest_name configured"
}

# Verify all routes on-chain
verify_routes() {
    local env=$1
    local env_file=$(get_env_file "$env")

    log_info "Verifying on-chain route configuration [env: $env]..."
    echo ""

    local all_aliases=($(get_all_chain_aliases))

    for source_alias in "${all_aliases[@]}"; do
        local source_chain_id=$(get_chain_id "$source_alias")
        local source_rpc=$(get_rpc_alias "$source_alias")
        local source_name=$(get_chain_name "$source_alias")
        local orderbook=$(get_orderbook_address "$source_chain_id")

        if [[ -z "$orderbook" ]]; then
            echo "$source_name: NOT DEPLOYED"
            continue
        fi

        echo "$source_name ($orderbook):"

        for dest_alias in "${all_aliases[@]}"; do
            if [[ "$source_alias" == "$dest_alias" ]]; then
                continue
            fi

            local dest_chain_id=$(get_chain_id "$dest_alias")
            local dest_name=$(get_chain_name "$dest_alias")

            if is_destination_configured "$env" "$source_rpc" "$orderbook" "$dest_chain_id"; then
                echo -e "  -> $dest_name (chainId: $dest_chain_id): ${GREEN}CONFIGURED${NC}"
            else
                echo -e "  -> $dest_name (chainId: $dest_chain_id): ${RED}NOT CONFIGURED${NC}"
            fi
        done
        echo ""
    done
}

# Show usage
usage() {
    echo "Configure bidirectional destination routes between OrderBooks"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod>                              Configure all bidirectional routes"
    echo "  $0 --env <dev|prod> --source <alias> --dest <alias>  Configure a single route"
    echo "  $0 --env <dev|prod> --verify                     Verify on-chain route configuration"
    echo ""
    echo "Examples:"
    echo "  $0 --env dev"
    echo "  $0 --env dev --source sepolia --dest arbitrum_sepolia"
    echo "  $0 --env prod --verify"
    echo ""
    echo "Environment files:"
    echo "  .env.dev   - Development/testnet configuration"
    echo "  .env.prod  - Production/mainnet configuration"
    echo ""
    echo "Secrets are managed via 1Password CLI (op)."
    echo "Account: $OP_ACCOUNT"
}

# Main
main() {
    check_dependencies

    local env=""
    local source_alias=""
    local dest_alias=""
    local verify_only=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --env|-e)
                env="$2"
                shift 2
                ;;
            --source|-s)
                source_alias="$2"
                shift 2
                ;;
            --dest|-d)
                dest_alias="$2"
                shift 2
                ;;
            --verify|-v)
                verify_only=true
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

    # Validate environment is specified
    if [[ -z "$env" ]]; then
        log_error "Environment is required. Use --env dev or --env prod"
        usage
        exit 1
    fi

    validate_env "$env"

    if [[ "$verify_only" == "true" ]]; then
        verify_routes "$env"
        exit 0
    fi

    if [[ -n "$source_alias" && -n "$dest_alias" ]]; then
        # Configure single route
        configure_route "$env" "$source_alias" "$dest_alias"
    elif [[ -n "$source_alias" || -n "$dest_alias" ]]; then
        log_error "Both --source and --dest must be specified together"
        exit 1
    else
        # Configure all bidirectional routes
        log_info "Configuring all bidirectional routes [env: $env]..."
        echo ""

        local all_aliases=($(get_all_chain_aliases))

        for source_alias in "${all_aliases[@]}"; do
            for dest_alias in "${all_aliases[@]}"; do
                if [[ "$source_alias" != "$dest_alias" ]]; then
                    configure_route "$env" "$source_alias" "$dest_alias"
                fi
            done
        done

        echo ""
        log_info "All routes configured!"
        echo ""
        verify_routes "$env"
    fi
}

main "$@"

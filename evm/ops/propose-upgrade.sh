#!/bin/bash
# Deploy a new OrderBook implementation and propose the upgrade (ProxyAdmin.upgradeAndCall)
# to the Safe multisig via the Safe Transaction Service.
#
# Usage: ./ops/propose-upgrade.sh --env prod --chain mainnet [--verify]
#
# Required environment variables (export in shell or add to .env.<env>):
#   SAFE_ADDRESS            Safe multisig (owns the ProxyAdmin)
#   PROPOSER_PRIVATE_KEY    Safe owner/proposer key (software signing), OR
#   PROPOSER_ADDRESS        Proposer address + LEDGER_DERIVATION_PATH (Ledger signing)
#
# Optional:
#   SAFE_NONCE=<n>          Explicit Safe nonce, to queue multiple proposals
#   DRY_RUN=true            Simulate without broadcasting or submitting the proposal
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVM_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE=""  # Set after env is parsed: chains.dev.json or chains.prod.json

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

# Check that a variable is set in the shell or defined in the env file
require_var() {
    local env_file=$1
    local name=$2
    if [[ -z "${!name:-}" ]] && ! grep -qE "^${name}=" "$env_file"; then
        return 1
    fi
    return 0
}

# Validate Safe configuration
validate_safe_config() {
    local env_file=$1

    if ! require_var "$env_file" "SAFE_ADDRESS"; then
        log_error "SAFE_ADDRESS is not set."
        log_error "Export it (export SAFE_ADDRESS=0x...) or add it to $env_file"
        exit 1
    fi

    if ! require_var "$env_file" "PROPOSER_PRIVATE_KEY" && ! require_var "$env_file" "PROPOSER_ADDRESS"; then
        log_error "No proposer configured. Set one of:"
        log_error "  PROPOSER_PRIVATE_KEY  - Safe owner/proposer key (software signing)"
        log_error "  PROPOSER_ADDRESS      - with LEDGER_DERIVATION_PATH for Ledger signing"
        exit 1
    fi

    if ! require_var "$env_file" "PROPOSER_PRIVATE_KEY" && ! require_var "$env_file" "LEDGER_DERIVATION_PATH"; then
        log_warn "PROPOSER_ADDRESS is set without LEDGER_DERIVATION_PATH."
        log_warn "Signing will fail unless the proposer key is otherwise available."
    fi
}

# Propose an upgrade on a single chain
propose_upgrade() {
    local env=$1
    local alias=$2
    local verify=${3:-false}
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

    validate_safe_config "$env_file"

    log_step "Proposing upgrade on $chain_name (chainId: $chain_id) [env: $env]"
    log_info "OrderBook proxy: $orderbook"

    # Build forge command (--ffi is required for the Safe Transaction Service HTTP calls)
    local forge_cmd="FOUNDRY_PROFILE=production forge script script/admin/ProposeUpgrade.s.sol:ProposeUpgrade"
    forge_cmd="$forge_cmd --rpc-url $rpc_alias"
    forge_cmd="$forge_cmd -vvv"
    forge_cmd="$forge_cmd --ffi"
    # Ignore assembly NatSpec memory-safe deprecation warnings from forge-std
    forge_cmd="$forge_cmd --ignored-error-codes 2424"

    # The new implementation deployment is broadcast by the deployer;
    # only the upgradeAndCall goes through the Safe.
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_warn "DRY RUN MODE - Simulating (no broadcast, no Safe proposal)"
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

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Dry run complete for $chain_name"
    else
        log_info "Proposal submitted. Review and confirm in the Safe UI before execution."
        log_warn "Remember to update deployments/$chain_id.json with the new implementation"
        log_warn "address after the Safe transaction has been executed."
    fi
}

# Show usage
usage() {
    echo "Deploy a new OrderBook implementation and propose the upgrade to the Safe multisig"
    echo ""
    echo "Usage:"
    echo "  $0 --env <dev|prod> --chain <alias>           Deploy impl and propose upgrade"
    echo "  $0 --env <dev|prod> --chain <alias> --verify  Also verify the implementation on the explorer"
    echo ""
    echo "Required environment variables:"
    echo "  SAFE_ADDRESS            Safe multisig address (owns the ProxyAdmin)"
    echo "  PROPOSER_PRIVATE_KEY    Safe owner/proposer key (software signing), or"
    echo "  PROPOSER_ADDRESS        Proposer address + LEDGER_DERIVATION_PATH (Ledger)"
    echo ""
    echo "Options:"
    echo "  SAFE_NONCE=<n>  Explicit Safe nonce (to queue multiple proposals)"
    echo "  DRY_RUN=true    Simulate without broadcasting or submitting the proposal"
    echo ""
    echo "Examples:"
    echo "  export SAFE_ADDRESS=0x... PROPOSER_PRIVATE_KEY=0x..."
    echo "  $0 --env prod --chain mainnet"
    echo "  $0 --env prod --chain mainnet --verify"
    echo ""
    echo "Secrets are managed via 1Password CLI (op)."
    echo "Account: $OP_ACCOUNT"
}

# Main
main() {
    check_dependencies

    local env=""
    local chain=""
    local verify=false

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
            --verify|-v)
                verify=true
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

    validate_env "$env"
    CONFIG_FILE="$EVM_DIR/config/chains.${env}.json"

    propose_upgrade "$env" "$chain" "$verify"
}

main "$@"

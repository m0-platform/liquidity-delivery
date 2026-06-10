#!/bin/bash
# Create a test order on the SVM OrderBook
# Usage: ./ops/open-order.sh --env devnet \
#            --token-in <mint_pubkey> --amount-in 1000000 \
#            --dest-chain-id 1 --token-out 0x... --amount-out 1000000
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SVM_DIR="$(dirname "$SCRIPT_DIR")"

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
    if ! command -v op &> /dev/null; then
        log_error "1Password CLI (op) is required but not installed. See: https://developer.1password.com/docs/cli"
        exit 1
    fi
    if ! command -v node &> /dev/null; then
        log_error "node is required but not installed."
        exit 1
    fi
    if ! command -v pnpm &> /dev/null; then
        log_error "pnpm is required but not installed."
        exit 1
    fi
}

# Show usage
usage() {
    echo "Create a test order on the SVM OrderBook"
    echo ""
    echo "Usage:"
    echo "  $0 --env <devnet|mainnet> \\"
    echo "     --token-in <mint_pubkey> --amount-in <amount> \\"
    echo "     --dest-chain-id <chain_id> --token-out <bytes32> --amount-out <amount> \\"
    echo "     [--recipient <bytes32>] [--solver <bytes32>] [--deadline <seconds>]"
    echo ""
    echo "Required arguments:"
    echo "  --env             Environment (devnet or mainnet)"
    echo "  --token-in        Input token mint address (Solana pubkey)"
    echo "  --amount-in       Amount of input token (in smallest unit)"
    echo "  --dest-chain-id   Destination chain ID (numeric)"
    echo "  --token-out       Output token address on destination (bytes32 hex)"
    echo "  --amount-out      Amount of output token expected (in smallest unit)"
    echo ""
    echo "Optional arguments:"
    echo "  --recipient       Recipient address on destination (bytes32 hex, defaults to zero)"
    echo "  --solver          Designated solver (bytes32 hex, zero = any solver)"
    echo "  --deadline        Deadline offset in seconds (default: 3600 = 1 hour)"
    echo ""
    echo "Environment variables:"
    echo "  DRY_RUN=true      Simulate without broadcasting"
    echo ""
    echo "Examples:"
    echo "  $0 --env devnet \\"
    echo "     --token-in EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v --amount-in 1000000 \\"
    echo "     --dest-chain-id 1 --token-out 0xa0b8...1234 --amount-out 1000000000000000000"
    echo ""
    echo "  DRY_RUN=true $0 --env devnet ..."
}

# Main
main() {
    check_dependencies

    local env=""
    local token_in=""
    local amount_in=""
    local dest_chain_id=""
    local token_out=""
    local amount_out=""
    local recipient=""
    local solver=""
    local deadline=""

    while [[ $# -gt 0 ]]; do
        case $1 in
            --env|-e)
                env="$2"
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
            --dest-chain-id)
                dest_chain_id="$2"
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
                recipient="$2"
                shift 2
                ;;
            --solver)
                solver="$2"
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
        log_error "Environment is required. Use --env devnet or --env mainnet"
        usage
        exit 1
    fi
    if [[ "$env" != "devnet" && "$env" != "mainnet" ]]; then
        log_error "Invalid environment: $env. Use devnet or mainnet"
        exit 1
    fi
    if [[ -z "$token_in" ]]; then
        log_error "Token in is required. Use --token-in <mint_pubkey>"
        exit 1
    fi
    if [[ -z "$amount_in" ]]; then
        log_error "Amount in is required. Use --amount-in <amount>"
        exit 1
    fi
    if [[ -z "$dest_chain_id" ]]; then
        log_error "Destination chain ID is required. Use --dest-chain-id <chain_id>"
        exit 1
    fi
    if [[ -z "$token_out" ]]; then
        log_error "Token out is required. Use --token-out <bytes32>"
        exit 1
    fi
    if [[ -z "$amount_out" ]]; then
        log_error "Amount out is required. Use --amount-out <amount>"
        exit 1
    fi

    # Resolve RPC URL env var name based on environment
    local rpc_var
    if [[ "$env" == "devnet" ]]; then
        rpc_var="HELIUS_DEV_RPC"
    else
        rpc_var="HELIUS_PROD_RPC"
    fi

    # Resolve keypair path from 1Password
    local keypair_path
    keypair_path=$(op read "op://Solana Dev/OrderBook Sender ${env^}/keypair_path" --account="$OP_ACCOUNT" 2>/dev/null || true)
    if [[ -z "$keypair_path" ]]; then
        # Fallback: use default Solana keypair
        keypair_path="$HOME/.config/solana/id.json"
        log_warn "1Password keypair not found — falling back to $keypair_path"
    fi

    if [[ ! -f "$keypair_path" ]]; then
        log_error "Keypair file not found: $keypair_path"
        exit 1
    fi

    log_step "Opening order on Solana ($env) -> Chain $dest_chain_id"
    log_info "Token In:        $token_in"
    log_info "Amount In:       $amount_in"
    log_info "Dest Chain ID:   $dest_chain_id"
    log_info "Token Out:       $token_out"
    log_info "Amount Out:      $amount_out"

    # Build tsx command
    local tsx_cmd="pnpm exec tsx scripts/open-order.ts"
    tsx_cmd="$tsx_cmd --token-in $token_in --amount-in $amount_in"
    tsx_cmd="$tsx_cmd --dest-chain-id $dest_chain_id --token-out $token_out --amount-out $amount_out"
    tsx_cmd="$tsx_cmd --keypair $keypair_path"
    tsx_cmd="$tsx_cmd --rpc-url \$$rpc_var"

    [[ -n "$recipient" ]] && tsx_cmd="$tsx_cmd --recipient $recipient"
    [[ -n "$solver" ]] && tsx_cmd="$tsx_cmd --solver $solver"
    [[ -n "$deadline" ]] && tsx_cmd="$tsx_cmd --deadline $deadline"

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        tsx_cmd="$tsx_cmd --dry-run"
        log_warn "DRY RUN MODE - Simulating order creation (no broadcast)"
    fi

    log_info "Running with 1Password: op run --env-file=.env.svm --account=$OP_ACCOUNT"

    # Execute from SVM directory with op run to resolve RPC secrets
    cd "$SVM_DIR"
    op run --env-file=.env.svm --account="$OP_ACCOUNT" -- bash -c "$tsx_cmd"

    if [[ "${DRY_RUN:-false}" != "true" ]]; then
        log_info "Order creation complete!"
    fi
}

main "$@"

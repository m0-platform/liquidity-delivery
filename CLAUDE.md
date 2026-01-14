# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the M0 Liquidity Delivery protocol - an intent-based limit order system for cross-chain token swaps. The protocol is implemented for both EVM and Solana (SVM) blockchains.

## Repository Structure

- `evm/` - Solidity implementation using Foundry
- `svm/` - Rust/Anchor implementation for Solana
- Root uses pnpm workspaces to manage both packages

## Build and Test Commands

### EVM (in `evm/` directory)

```bash
# Build
make build

# Run all tests
make tests

# Run tests matching a contract name
forge test --mc <ContractName>

# Run a specific test
forge test --mt <test_function_name>

# Run tests with verbose output
./test.sh -v

# Code formatting
pnpm run prettier

# Linting
pnpm run solhint
pnpm run solhint-fix
```

### EVM Deployment (in `evm/` directory)

Deployment uses 1Password CLI for secret management. All commands require `ENV=dev` (testnet) or `ENV=prod` (mainnet).

```bash
# Deploy to a single chain
make deploy ENV=dev CHAIN=sepolia

# Deploy to all configured chains
make deploy-all ENV=dev

# Configure cross-chain routes (bidirectional)
make configure-routes ENV=dev

# Verify on-chain route configuration
make verify-routes ENV=dev

# Upgrade implementation
make upgrade ENV=dev CHAIN=sepolia

# Show deployment status
make status
make help
```

See `evm/CLAUDE.md` for detailed deployment documentation.

### SVM (in `svm/` directory)

```bash
# Build
anchor build

# Run tests
anchor test

# Run specific Rust tests
cargo test <test_name>
```

### Root-level

```bash
# Install dependencies
pnpm install
```

## Architecture

### Core Concept: OrderBook

Both implementations center on an OrderBook contract/program that manages limit orders for cross-chain token swaps:

1. **Order Creation** - Users deposit tokens and create orders specifying destination chain, output token, amounts, deadline, and optional exclusive solver
2. **Order Filling** - Solvers fill orders on the destination chain by providing output tokens to recipients
3. **Cross-chain Messaging** - Fill/cancel reports are sent via Portal V2 (M0's cross-chain messaging protocol) back to the origin chain to release escrowed funds

### Key Order Types

- **Native Orders** - Orders created on the current chain (full order data stored)
- **Foreign Orders** - Orders created on another chain that need to be filled here (only fill status tracked)

### Order ID Computation

Order IDs are computed identically on both chains using keccak256 hash of packed order data. The `OrderData` struct encoding must match exactly between EVM and SVM - see `svm/programs/order_book/src/state/orders.rs` for the cross-chain compatibility test.

### EVM Structure (`evm/src/`)

- `OrderBook.sol` - Main contract (upgradeable, uses ERC-7201 storage pattern)
- `interfaces/IOrderBook.sol` - Interface with all events, errors, and structs
- `interfaces/IPortalV2Like.sol` - Portal interface for cross-chain messaging
- Uses OpenZeppelin upgradeable contracts and M0's common library (`lib/common/`)

### SVM Structure (`svm/programs/order_book/src/`)

- `lib.rs` - Program entry point with all instruction handlers
- `instructions/` - Instruction implementations (open, fill, cancel, destination reports)
- `state/orders.rs` - Order account structures and ID computation
- `error.rs` - Custom error definitions
- Uses Anchor framework 0.31.1 with `anchor-litesvm` for testing

### Cross-chain Flow

1. User opens order on origin chain (funds escrowed)
2. Solver fills order on destination chain (pays recipient)
3. Destination chain sends `FillReport` via Portal to origin
4. Origin chain releases escrowed funds to solver

Cancellation follows similar pattern with `CancelReport` enabling refunds.

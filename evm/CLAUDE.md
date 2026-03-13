# EVM CLAUDE.md

This file provides guidance for working with the EVM (Solidity) implementation of the OrderBook protocol.

## Build and Test

```bash
# Build contracts
make build

# Run all tests
make tests

# Run tests matching a contract name
forge test --mc <ContractName>

# Run a specific test
forge test --mt <test_function_name>

# Code formatting and linting
pnpm run prettier
pnpm run solhint
```

## Multi-Chain Deployment System

The deployment system uses bash scripts with 1Password CLI (`op`) for secret management. All secrets are stored in 1Password and referenced in `.env.dev` and `.env.prod` files.

### Prerequisites

- Foundry toolchain (`forge`, `cast`)
- 1Password CLI (`op`) - https://developer.1password.com/docs/cli
- `jq` for JSON parsing

### Configuration Files

| File | Purpose |
|------|---------|
| `config/chains.dev.json` | Testnet chain registry (chainId, RPC alias, explorer alias) |
| `config/chains.prod.json` | Mainnet chain registry (chainId, RPC alias, explorer alias) |
| `.env.dev` | Testnet secrets (1Password references) |
| `.env.prod` | Mainnet secrets (1Password references) |
| `deployments/{chainId}.json` | Deployed contract addresses per chain |

### Environment Files

Environment files contain 1Password secret references, not actual secrets:

```bash
# .env.dev example
PRIVATE_KEY="op://Engineering/OrderBook Deployer Dev/private_key"
ADMIN_ADDRESS="op://Engineering/OrderBook Deployer Dev/admin_address"
SEPOLIA_RPC_URL="op://Engineering/RPC URLs Dev/sepolia"
```

### Makefile Commands

All deployment commands require `ENV=dev` or `ENV=prod`.

#### Deployment

```bash
# Deploy to a single chain
make deploy ENV=dev CHAIN=sepolia
make deploy ENV=dev CHAIN=arbitrum_sepolia

# Deploy with contract verification
make deploy-verify ENV=dev CHAIN=sepolia

# Deploy to all configured chains
make deploy-all ENV=dev

# List configured chains and deployment status
make deploy-list
make status
```

#### Route Configuration

After deploying to multiple chains, configure bidirectional routes:

```bash
# Configure all routes between deployed chains
make configure-routes ENV=dev

# Configure a single route
make configure-dest ENV=dev CHAIN=sepolia DEST=arbitrum_sepolia

# Verify on-chain route configuration
make verify-routes ENV=dev
```

#### Upgrades

```bash
# Upgrade implementation on a single chain
make upgrade ENV=dev CHAIN=sepolia

# Upgrade with verification
make upgrade-verify ENV=dev CHAIN=sepolia

# Upgrade on all deployed chains
make upgrade-all ENV=dev

# Show current implementation status
make upgrade-status
```

### Direct Script Usage

Scripts can also be called directly:

```bash
# Deploy
./ops/deploy.sh --env dev --chain sepolia
./ops/deploy.sh --env dev --all --verify
./ops/deploy.sh --list

# Configure routes
./ops/configure-routes.sh --env dev
./ops/configure-routes.sh --env dev --source sepolia --dest arbitrum_sepolia
./ops/configure-routes.sh --env dev --verify

# Upgrade
./ops/upgrade.sh --env dev --chain sepolia
./ops/upgrade.sh --env dev --all
./ops/upgrade.sh --status
```

### Adding a New Chain

1. Add chain entry to the appropriate config file (`config/chains.dev.json` for testnets, `config/chains.prod.json` for mainnets):
   ```json
   {
     "chains": {
       "new_chain": {
         "chainId": 12345,
         "name": "New Chain",
         "rpcAlias": "new_chain",
         "explorerAlias": "new_chain"
       }
     }
   }
   ```

2. Add RPC endpoint to `foundry.toml`:
   ```toml
   [rpc_endpoints]
   new_chain = "${NEW_CHAIN_RPC_URL}"

   [etherscan]
   new_chain = { key = "${ETHERSCAN_API_KEY}", url = "${NEW_CHAIN_VERIFIER_URL}" }
   ```

3. Add RPC URL to appropriate `.env.*` file:
   ```bash
   NEW_CHAIN_RPC_URL="op://Engineering/RPC URLs Dev/new_chain"
   ```

4. Deploy and configure:
   ```bash
   make deploy ENV=dev CHAIN=new_chain
   make configure-routes ENV=dev
   ```

### 1Password Setup

Create these items in 1Password (vault: Engineering):

**OrderBook Deployer Dev:**
- `private_key` - Deployer private key (with 0x prefix)
- `admin_address` - Admin address for OrderBook roles

**OrderBook Deployer Prod:**
- Same fields as Dev, for production keys

**RPC URLs Dev:**
- `sepolia` - Sepolia RPC URL
- `arbitrum_sepolia` - Arbitrum Sepolia RPC URL
- Add fields for each testnet

**RPC URLs Prod:**
- `mainnet` - Mainnet RPC URL
- `arbitrum` - Arbitrum RPC URL
- Add fields for each mainnet

**Etherscan API:**
- `api_key` - Etherscan API key for verification

### Deployment Workflow Example

```bash
# 1. Deploy to testnets
make deploy ENV=dev CHAIN=sepolia
make deploy ENV=dev CHAIN=arbitrum_sepolia

# 2. Configure bidirectional routes
make configure-routes ENV=dev

# 3. Verify configuration
make verify-routes ENV=dev

# 4. Check status
make status
```

## Contract Architecture

### Proxy Pattern

OrderBook uses OpenZeppelin's `TransparentUpgradeableProxy`:
- Proxy deployed via CREATE3 (deterministic address)
- ProxyAdmin created automatically, owned by `ADMIN_ADDRESS`
- Upgrades go through ProxyAdmin.upgradeAndCall()

### Key Contracts

| Contract | Purpose |
|----------|---------|
| `OrderBook.sol` | Main implementation (upgradeable) |
| `IOrderBook.sol` | Interface with events, errors, structs |
| `IPortalV2Like.sol` | Portal interface for cross-chain messaging |

### Admin Functions

```solidity
// Set destination chain support
function setDestinationSupported(uint32 destChainId, bool isSupported) external;

// Pause/unpause
function pause() external;
function unpause() external;
```

### Deployment Scripts

| Script | Purpose |
|--------|---------|
| `script/deploy/Deploy.s.sol` | Initial deployment via CREATE3 |
| `script/deploy/Upgrade.s.sol` | Upgrade implementation via ProxyAdmin |
| `script/config/ConfigureDestination.s.sol` | Set destination chain support |

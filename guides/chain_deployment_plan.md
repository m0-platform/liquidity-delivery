# Single Chain Deployment Checklist

This document describes every step required to deploy the OrderBook protocol to a new production chain. Follow the relevant section depending on whether you are deploying an EVM chain or the SVM (Solana) program.

---

## EVM Chain Deployment

Fill in the chain parameters below, then work through each step in order.

### Chain Parameters

| Parameter                     | Value                                                   |
| ----------------------------- | ------------------------------------------------------- |
| **Chain name**                | _(e.g. Base)_                                           |
| **Chain alias**               | _(e.g. `base` - used in config files and Makefile)_     |
| **Chain ID**                  | _(e.g. `8453`)_                                         |
| **RPC env var name**          | _(e.g. `BASE_RPC_URL`)_                                 |
| **Verifier URL**              | _(e.g. `https://api.etherscan.io/v2/api?chainid=8453`)_ |
| **Verifier URL env var name** | _(e.g. `BASE_VERIFIER_URL`)_                            |

### Step 1: Prerequisites

- [ ] `forge`, `cast`, `jq`, and `op` (1Password CLI) are installed
- [ ] Authenticated to 1Password: `op signin --account mzerolabs.1password.com`
- [ ] On the correct git commit (audited release)

### Step 2: Add chain to the appropriate chain config

Add an entry under `"chains"` in the environment-specific config file:
- **Testnets:** `evm/config/chains.dev.json`
- **Mainnets:** `evm/config/chains.prod.json`

```json
"<chain_alias>": {
  "chainId": <chain_id>,
  "name": "<Chain Name>",
  "rpcAlias": "<chain_alias>",
  "explorerAlias": "<chain_alias>"
}
```

- [ ] Entry added to the appropriate `evm/config/chains.<env>.json`

### Step 3: Add RPC endpoint to `evm/foundry.toml`

Under `[rpc_endpoints]`:

```toml
<chain_alias> = "${<RPC_ENV_VAR_NAME>}"
```

- [ ] RPC endpoint added to `[rpc_endpoints]`

### Step 4: Add Etherscan config to `evm/foundry.toml`

Under `[etherscan]`:

```toml
<chain_alias> = { key = "${ETHERSCAN_API_KEY}", url = "${<VERIFIER_URL_ENV_VAR_NAME>}" }
```

- [ ] Etherscan entry added to `[etherscan]`

### Step 5: Add environment variables to `evm/.env.prod`

Add the RPC URL 1Password reference:

```bash
<RPC_ENV_VAR_NAME>="op://<vault>/<item>/<field>"
```

Add the verifier URL (if not already present):

```bash
<VERIFIER_URL_ENV_VAR_NAME>="https://api.etherscan.io/v2/api?chainid=<chain_id>"
```

- [ ] RPC URL reference added to `.env.prod`
- [ ] Verifier URL added to `.env.prod` (if needed)

### Step 6: Verify all required `.env.prod` variables exist in 1Password

The `Deploy.s.sol` script reads the following environment variables. Every 1Password reference (`op://...`) must resolve.

| Variable               | 1Password Reference                                      | Used By                                                           |
| ---------------------- | -------------------------------------------------------- | ----------------------------------------------------------------- |
| `DEPLOYER_PRIVATE_KEY` | `op://Protocol Accounts/Protocol One LTD/Private key`    | `Deploy.s.sol` - signs deployment tx                              |
| `ADMIN_ADDRESS`        | `op://Protocol Accounts/Protocol One LTD/Address`        | `Deploy.s.sol` - receives `DEFAULT_ADMIN_ROLE`, owns `ProxyAdmin` |
| `PAUSER_ADDRESS`       | `op://Protocol Accounts/Protocol One LTD/Address`        | `Deploy.s.sol` - receives `PAUSER_ROLE`                           |
| `PORTAL_ADDRESS`       | `0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd` (hardcoded) | `Deploy.s.sol` - Portal V2 contract                               |
| `ETHERSCAN_API_KEY`    | `op://Web3/Etherscan/API KEY`                            | Foundry verification                                              |
| `<RPC_ENV_VAR_NAME>`   | `op://<vault>/<item>/<field>`                            | Foundry RPC connection                                            |

- [ ] `DEPLOYER_PRIVATE_KEY` resolves in 1Password
- [ ] `ADMIN_ADDRESS` resolves in 1Password
- [ ] `PAUSER_ADDRESS` resolves in 1Password
- [ ] `ETHERSCAN_API_KEY` resolves in 1Password
- [ ] `<RPC_ENV_VAR_NAME>` resolves in 1Password
- [ ] `PORTAL_ADDRESS` is correct for mainnet (`0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`)

_IMPORTANT_: You must use the correct deployer wallet to get the same deterministic address for the `OrderBook` on all chains because the calculated salt is guarded by the Protocol One deployer address.

### Step 7: Team review

- [ ] **Admin address** confirmed with team - this address will own the `ProxyAdmin` and hold `DEFAULT_ADMIN_ROLE` (controls `setDestinationSupported`, role grants/revokes)
- [ ] **Pauser address** confirmed with team - this address holds `PAUSER_ROLE` (controls `pause()`/`unpause()`)
- [ ] **Portal V2 address** confirmed for this chain (same on all EVM mainnet chains: `0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`)
- [ ] **Deployer wallet funded** - deployer address has sufficient native gas token on the target chain
- [ ] **Commit hash** agreed upon by team

### Step 8: Verify chain appears in deployment list

```bash
cd evm
make deploy-list
```

- [ ] New chain appears as **NOT DEPLOYED**

### Step 9: Dry-run deployment

Simulate the deployment without broadcasting to verify configuration is correct:

```bash
DRY_RUN=true make deploy ENV=prod CHAIN=<chain_alias>
```

In dry-run mode, the script skips the "already deployed" check and does not broadcast transactions. The deployment JSON is logged to the console instead of being written to a file.

- [ ] Dry run completes without errors
- [ ] Logged deployment JSON shows expected addresses

### Step 10: Deploy OrderBook

```bash
make deploy-verify ENV=prod CHAIN=<chain_alias>
```

This executes:

1. `FOUNDRY_PROFILE=production forge script script/deploy/Deploy.s.sol` via `op run`
2. Deploys the `OrderBook` implementation contract (constructor arg: Portal address)
3. Deploys a `TransparentUpgradeableProxy` via CREATE3 (deterministic address based on deployer + salt)
4. Calls `OrderBook.initialize(admin, pauser)` through the proxy
5. Verifies the contracts on Etherscan
6. Writes deployment details to `evm/deployments/<chain_id>.json`

- [ ] Deployment transaction confirmed on block explorer
- [ ] `evm/deployments/<chain_id>.json` created with `orderBook`, `implementation`, `proxyAdmin`, and `upgradedAt` fields

### Step 11: Post-deployment verification

Run `make deploy-list` and confirm the new chain shows its OrderBook address.

```bash
make deploy-list
```

- [ ] Chain shows deployed OrderBook address

Verify on-chain state using `cast` (substitute actual addresses and RPC):

```bash
# Verify DEFAULT_ADMIN_ROLE holder (bytes32(0) is the default admin role)
cast call <orderbook_proxy> "hasRole(bytes32,address)(bool)" \
  0x0000000000000000000000000000000000000000000000000000000000000000 \
  <admin_address> --rpc-url <rpc_url>
```

- [ ] Returns `true`

```bash
# Verify PAUSER_ROLE holder
cast call <orderbook_proxy> "hasRole(bytes32,address)(bool)" \
  $(cast keccak "PAUSER_ROLE") \
  <pauser_address> --rpc-url <rpc_url>
```

- [ ] Returns `true`

```bash
# Verify Portal address
cast call <orderbook_proxy> "portal()(address)" --rpc-url <rpc_url>
```

- [ ] Returns `0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`

```bash
# Verify contract is not paused
cast call <orderbook_proxy> "paused()(bool)" --rpc-url <rpc_url>
```

- [ ] Returns `false`

### Step 12: Commit deployment artifact

- [ ] `evm/deployments/<chain_id>.json` committed to the repository
- [ ] Config file changes (`chains.<env>.json`, `foundry.toml`, `.env.prod`) committed to the repository
- [ ] Deployed proxy address communicated to the team

### Step 13: (When Ready) Configure destination routes and upgrades

`ADMIN_PRIVATE_KEY` is required in `.env.prod` for both route configuration (`ConfigureDestination.s.sol`) and contract upgrades (`Upgrade.s.sol`). This is the private key corresponding to `ADMIN_ADDRESS` (the `ProxyAdmin` owner / `DEFAULT_ADMIN_ROLE` holder).

Ensure `.env.prod` contains:

```bash
ADMIN_PRIVATE_KEY="op://<vault>/<item>/<field>"
```

For each destination chain that should be reachable from this chain:

```bash
make configure-dest ENV=prod CHAIN=<this_chain_alias> DEST=<destination_alias>
```

And the reverse direction:

```bash
make configure-dest ENV=prod CHAIN=<destination_alias> DEST=<this_chain_alias>
```

Verify all routes:

```bash
make verify-routes ENV=prod
```

- [ ] `ADMIN_PRIVATE_KEY` added to `.env.prod` and resolves in 1Password
- [ ] Outbound route configured (this chain -> destination)
- [ ] Inbound route configured (destination -> this chain)
- [ ] `make verify-routes ENV=prod` shows all routes as **CONFIGURED**

### Step 14: (When Ready) Transfer roles to new admin

After deployment and initial configuration, transfer all privileged roles from the deployer wallet to a new admin address (e.g. a Safe multisig). This uses the `TransferRoles.s.sol` script which handles all transfers in a single broadcast. It requires `ADMIN_PRIVATE_KEY` in `.env.prod`.

The script transfers three privileged roles in the correct order:

1. Grants `DEFAULT_ADMIN_ROLE` to the new admin
2. Grants `PAUSER_ROLE` to the new admin
3. Renounces `PAUSER_ROLE` from the deployer
4. Renounces `DEFAULT_ADMIN_ROLE` from the deployer
5. Transfers `ProxyAdmin` ownership to the new admin

The script is idempotent — it checks on-chain state before each operation and skips already-completed steps.

#### 14a. Dry-run the transfer

```bash
cd evm
DRY_RUN=true make transfer-roles ENV=prod CHAIN=<chain_alias> NEW_ADMIN=<new_admin_address>
```

- [ ] Dry run completes without errors
- [ ] Console output shows current state and planned operations

#### 14b. Execute the transfer

```bash
cd evm
make transfer-roles ENV=prod CHAIN=<chain_alias> NEW_ADMIN=<new_admin_address>
```

- [ ] All transactions confirmed on block explorer

#### 14c. Verify the transfer

```bash
# Verify new admin has DEFAULT_ADMIN_ROLE
cast call <orderbook_proxy> "hasRole(bytes32,address)(bool)" \
  0x0000000000000000000000000000000000000000000000000000000000000000 \
  <new_admin_address> --rpc-url <rpc_url>
```

- [ ] Returns `true`

```bash
# Verify new admin has PAUSER_ROLE
cast call <orderbook_proxy> "hasRole(bytes32,address)(bool)" \
  $(cast keccak "PAUSER_ROLE") \
  <new_admin_address> --rpc-url <rpc_url>
```

- [ ] Returns `true`

```bash
# Verify deployer no longer has DEFAULT_ADMIN_ROLE
cast call <orderbook_proxy> "hasRole(bytes32,address)(bool)" \
  0x0000000000000000000000000000000000000000000000000000000000000000 \
  <deployer_address> --rpc-url <rpc_url>
```

- [ ] Returns `false`

```bash
# Verify ProxyAdmin owned by new admin (proxyAdmin address from deployments/<chain_id>.json)
cast call <proxy_admin_address> "owner()(address)" --rpc-url <rpc_url>
```

- [ ] Returns `<new_admin_address>`

---

## SVM (Solana) Chain Deployment

### Chain Parameters

| Parameter                  | Value                                          |
| -------------------------- | ---------------------------------------------- |
| **Environment name**       | `mainnet`                                      |
| **Network ID**             | `mainnet`                                      |
| **Chain ID**               | `1399811149`                                   |
| **Program ID**             | `MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK`  |
| **Portal program ID**      | `MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce`  |
| **Squads multisig**        | `BKEgAKFUBAYee5JuTAf6HjMHAiFLqvNtAKxxZb5aay5F` |
| **Squads create key**      | `Ga8eDs74ZjWx7Prbm5Yu9pBc91YZgLqdURM4aK3HcYrX` |
| **Payer keypair**          | `~/.config/solana/id.json`                     |
| **Expected payer address** | `D76ySoHPwD8U2nnTTDqXeUJQg5UkD9UD1PUE1rnvPAGm` |

### Step 1: Prerequisites

- [ ] `anchor`, `surfpool`, `solana` CLI, and `op` (1Password CLI) are installed
- [ ] Authenticated to 1Password: `op signin --account mzerolabs.1password.com`
- [ ] On the correct git commit (audited release)

### Step 2: Verify `svm/txtx.yml` has the target environment

The mainnet environment should already be defined:

```yaml
mainnet:
  network_id: mainnet
  rpc_api_url: "${HELIUS_PROD_RPC}"
  chain_id: 1399811149
  build_dir: target/verifiable
```

Secrets are injected at runtime via `op run --env-file=.env.svm` — the `txtx.yml` file contains no plaintext secrets.

- [ ] Environment block exists in `svm/txtx.yml`
- [ ] `chain_id` is correct (`1399811149` for Solana mainnet)
- [ ] `build_dir` is `target/verifiable` (enables verifiable builds)

### Step 3: Verify 1Password entries

The `op://` references in `svm/.env.svm` must resolve. These are injected as environment variables at runtime by `op run`.

| Item               | 1Password Reference               | Env var           | Purpose              |
| ------------------ | --------------------------------- | ----------------- | -------------------- |
| Helius devnet RPC  | `op://Solana Dev/Helius/dev rpc`  | `HELIUS_DEV_RPC`  | Devnet RPC endpoint  |
| Helius mainnet RPC | `op://Solana Dev/Helius/prod rpc` | `HELIUS_PROD_RPC` | Mainnet RPC endpoint |

- [ ] `op://Solana Dev/Helius/prod rpc` resolves in 1Password

### Step 4: Verify local payer keypair

```bash
solana address -k ~/.config/solana/id.json
```

- [ ] Output matches expected payer address: `D76ySoHPwD8U2nnTTDqXeUJQg5UkD9UD1PUE1rnvPAGm`

### Step 5: Verify payer has SOL for fees

```bash
solana balance D76ySoHPwD8U2nnTTDqXeUJQg5UkD9UD1PUE1rnvPAGm --url mainnet-beta
```

- [ ] Payer has sufficient SOL for program deployment fees (program deploys require significant rent + tx fees)

### Step 6: Verify Squads multisig access

The mainnet deployment and initialize runbooks use a Squads multisig as the authority. Both `deployment/signers.mainnet.tx` and `initialize/signers.mainnet.tx` require a web wallet initiator to approve transactions.

- Multisig account: `BKEgAKFUBAYee5JuTAf6HjMHAiFLqvNtAKxxZb5aay5F`
- Create key: `Ga8eDs74ZjWx7Prbm5Yu9pBc91YZgLqdURM4aK3HcYrX`

- [ ] Squads multisig exists on Solana mainnet
- [ ] A web wallet initiator has access to the multisig and is available to approve transactions

### Step 7: Verify signer files match runbook expectations

Each runbook (`deployment`, `initialize`, `add_destination`, `remove_destination`) has a `signers.mainnet.tx` file that defines the signers used by `main.tx`. Verify the signer names match:

| Runbook              | `main.tx` references               | `signers.mainnet.tx` defines      |
| -------------------- | ---------------------------------- | --------------------------------- |
| `deployment`         | `signer.authority`, `signer.payer` | `payer`, `initiator`, `authority` |
| `initialize`         | `signer.caller`                    | `initiator`, `caller`             |
| `add_destination`    | `signer.caller`                    | `initiator`, `caller`             |
| `remove_destination` | `signer.caller`                    | `initiator`, `caller`             |

- [ ] Signer names in `signers.mainnet.tx` match what each `main.tx` expects

### Step 8: Team review

- [ ] **Program ID** confirmed: `MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK` (declared in `svm/programs/order_book/src/lib.rs:26`)
- [ ] **Portal program ID** confirmed for mainnet: `MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce` (the portal authority PDA is derived from this)
- [ ] **Squads multisig members** are aware they will need to approve transactions via web wallet
- [ ] **Commit hash** agreed upon by team

### Step 9: Verify 1Password authentication

Secrets are injected at runtime by `op run` via `envsubst` into a temporary `raw.txtx.yml` file, which is cleaned up after each command. Verify you are authenticated:

```bash
op whoami --account mzerolabs.1password.com
```

- [ ] 1Password CLI is authenticated and can resolve references in `svm/.env.svm`

### Step 10: Build the program

```bash
anchor build -p order_book
```

- [ ] Build succeeds with no errors
- [ ] Verifiable build output exists in `target/verifiable/` (mainnet `build_dir`)

### Step 11: Deploy the program

```bash
make deploy env=mainnet
```

This injects secrets from 1Password into a temporary `raw.txtx.yml` via `envsubst`, then runs `surfpool run deployment -m raw.txtx.yml --env mainnet --unsupervised`, which:

1. Reads the built program from the Anchor project
2. Uses the local keypair (`~/.config/solana/id.json`) as the fee payer
3. Uses the Squads multisig as the program authority
4. Deploys the program on-chain

- [ ] Approve the Squads multisig transaction via web wallet when prompted
- [ ] Deployment transaction confirmed on Solana explorer
- [ ] Program ID on-chain matches `MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK`

### Step 12: Verify program deployment

```bash
solana program show MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK --url mainnet-beta
```

- [ ] Program exists on-chain
- [ ] Program authority is the Squads multisig vault address

### Step 13: Initialize the program

```bash
make initialize env=mainnet
```

This injects secrets from 1Password into a temporary `raw.txtx.yml` via `envsubst`, then runs `surfpool run initialize -m raw.txtx.yml --env mainnet --unsupervised`, which:

1. Checks if the `OrderBookGlobal` account already exists (skips if so)
2. Creates the global account with:
   - `admin`: public key derived from the caller signer (Squads multisig)
   - `chain_id`: `1399811149` (from `txtx.yml` environment)
   - `portal_authority`: PDA derived from Portal program (`MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce`, seed: `["authority"]`)
   - `paused`: `false`

- [ ] Approve the Squads multisig transaction via web wallet when prompted
- [ ] Initialization transaction confirmed on Solana explorer

### Step 14: Verify initialization

Query the global account on-chain and verify:

- [ ] `admin` is set to the Squads multisig vault address
- [ ] `chain_id` is `1399811149`
- [ ] `portal_authority` matches the expected PDA
- [ ] `paused` is `false`

### Step 15: Record deployment

- [ ] Deployed program address communicated to the team
- [ ] Any config changes committed to the repository

### Step 16: (When Ready) Add destination chains

This step is performed later when you are ready to enable cross-chain order flow. For each destination chain, edit the editable variables in `runbooks/add_destination/main.tx`:

- `destination_chain_id` - the numeric chain ID (e.g. `1` for Ethereum mainnet, `8453` for Base)
- `destination_chain_id_hex` - big-endian hex encoding (e.g. `0x00000001` for chain ID 1, `0x00002105` for chain ID 8453)

Then run:

```bash
make add-destination env=mainnet
```

Repeat for each destination chain that should be reachable.

- [ ] `destination_chain_id` and `destination_chain_id_hex` set correctly
- [ ] `make add-destination env=mainnet` succeeds for each destination
- [ ] Approve Squads multisig transaction for each destination

### Step 17: (When Ready) Transfer admin to team multisig

If the program was initialized with a non-multisig admin (e.g. the deployer keypair on devnet), transfer the admin role to the team multisig. On mainnet this may already be the Squads multisig from initialization — verify before proceeding.

The OrderBook program uses a **two-step admin transfer**:

1. Current admin calls `set_new_admin(new_admin)` — sets `new_admin` on the global account
2. New admin calls `accept_admin_role()` — completes the transfer

#### 17a. Initiate admin transfer

The current admin must call `set_new_admin` with the new admin's public key (the team multisig vault address):

```
order_book::set_new_admin(new_admin: <team_multisig_vault_pubkey>)
```

If the current admin is already the Squads multisig, this transaction must be proposed and approved through the multisig.

- [ ] `set_new_admin` transaction confirmed
- [ ] Verify `new_admin` is set on the global account

#### 17b. Accept admin role

The new admin must call `accept_admin_role()`. If the new admin is a multisig, this must be executed as a multisig transaction.

```
order_book::accept_admin_role()
```

- [ ] `accept_admin_role` transaction confirmed
- [ ] Verify `admin` on the global account is now `<team_multisig_vault_pubkey>`
- [ ] Verify `new_admin` is cleared (`None`)

**Note:** The program authority (upgrade authority) is separate from the OrderBook admin. The program authority is set during deployment (`signer.authority` in the deployment runbook) and is already the Squads multisig on mainnet. No transfer is needed for upgrade authority.

---

## Quick Reference

### Key Addresses

| Description                      | Address                                        |
| -------------------------------- | ---------------------------------------------- |
| EVM Portal V2 (mainnet)          | `0xD925C84b55E4e44a53749fF5F2a5A13F63D128fd`   |
| CREATE3 Factory (all EVM chains) | `0xba5Ed099633D3B313e4D5F7bdc1305d3c28ba5Ed`   |
| SVM order_book program           | `MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK`  |
| SVM Portal program               | `MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce`  |
| SVM Squads multisig              | `BKEgAKFUBAYee5JuTAf6HjMHAiFLqvNtAKxxZb5aay5F` |
| SVM payer                        | `D76ySoHPwD8U2nnTTDqXeUJQg5UkD9UD1PUE1rnvPAGm` |
| 1Password account                | `mzerolabs.1password.com`                      |

### EVM Deployment Artifacts

Each EVM deployment produces `evm/deployments/<chain_id>.json` with the full deployment record:

```json
{
  "orderBook": "0x...",
  "implementation": "0x...",
  "proxyAdmin": "0x...",
  "upgradedAt": 1234567890
}
```

After an upgrade, the same file is overwritten with the new implementation address and timestamp. The `orderBook` (proxy) and `proxyAdmin` addresses remain stable across upgrades.

### Config Files Modified Per EVM Chain

| File                     | Change                                          |
| ------------------------ | ----------------------------------------------- |
| `evm/config/chains.<env>.json` | Add chain entry                           |
| `evm/foundry.toml`       | Add `[rpc_endpoints]` and `[etherscan]` entries |
| `evm/.env.prod`          | Add RPC URL and verifier URL references         |

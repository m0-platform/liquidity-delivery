# include .env file and export its env vars
# (-include to ignore error if it does not exist)
-include .env

# dapp deps
update:; forge update

# Run slither
slither :; FOUNDRY_PROFILE=production forge build --build-info --skip '*/test/**' --skip '*/script/**' --force && slither --compile-force-framework foundry --ignore-compile --sarif results.sarif --config-file slither.config.json .

# Common tasks
profile ?=default

build:
	@./build.sh -p production

tests:
	@./test.sh -p $(profile)

coverage:
	FOUNDRY_PROFILE=$(profile) forge coverage --report lcov && lcov --extract lcov.info -o lcov.info 'src/*' && genhtml lcov.info -o coverage

gas-report:
	FOUNDRY_PROFILE=$(profile) forge test --gas-report > gasreport.ansi

sizes:
	@./build.sh -p production -s

clean:
	forge clean && rm -rf ./abi && rm -rf ./bytecode && rm -rf ./types

# =============================================================================
# Multi-Chain Deployment System (using 1Password CLI)
# =============================================================================
# All commands require ENV=dev or ENV=prod to specify the environment.
# Secrets are loaded via 1Password CLI from .env.dev or .env.prod files.

## Deployment Commands

.PHONY: deploy deploy-all deploy-list

deploy: ## Deploy to a chain (ENV=dev CHAIN=sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make deploy ENV=dev CHAIN=sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make deploy ENV=dev CHAIN=sepolia"
	@exit 1
endif
	@./ops/deploy.sh --env $(ENV) --chain $(CHAIN)

deploy-verify: ## Deploy to a chain with verification (ENV=dev CHAIN=sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make deploy-verify ENV=dev CHAIN=sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make deploy-verify ENV=dev CHAIN=sepolia"
	@exit 1
endif
	@./ops/deploy.sh --env $(ENV) --chain $(CHAIN) --verify

deploy-all: ## Deploy to all configured chains (ENV=dev)
ifndef ENV
	@echo "Error: ENV is required. Usage: make deploy-all ENV=dev"
	@exit 1
endif
	@./ops/deploy.sh --env $(ENV) --all

deploy-all-verify: ## Deploy to all chains with verification (ENV=dev)
ifndef ENV
	@echo "Error: ENV is required. Usage: make deploy-all-verify ENV=dev"
	@exit 1
endif
	@./ops/deploy.sh --env $(ENV) --all --verify

deploy-list: ## List all configured chains and deployment status
	@./ops/deploy.sh --list

## Route Configuration Commands

.PHONY: configure-routes configure-dest verify-routes

configure-routes: ## Configure all bidirectional routes (ENV=dev)
ifndef ENV
	@echo "Error: ENV is required. Usage: make configure-routes ENV=dev"
	@exit 1
endif
	@./ops/configure-routes.sh --env $(ENV)

configure-dest: ## Configure a single route (ENV=dev CHAIN=sepolia DEST=arbitrum_sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make configure-dest ENV=dev CHAIN=sepolia DEST=arbitrum_sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make configure-dest ENV=dev CHAIN=sepolia DEST=arbitrum_sepolia"
	@exit 1
endif
ifndef DEST
	@echo "Error: DEST is required. Usage: make configure-dest ENV=dev CHAIN=sepolia DEST=arbitrum_sepolia"
	@exit 1
endif
	@./ops/configure-routes.sh --env $(ENV) --source $(CHAIN) --dest $(DEST)

verify-routes: ## Verify on-chain route configuration (ENV=dev)
ifndef ENV
	@echo "Error: ENV is required. Usage: make verify-routes ENV=dev"
	@exit 1
endif
	@./ops/configure-routes.sh --env $(ENV) --verify

## Upgrade Commands

.PHONY: upgrade upgrade-all upgrade-status

upgrade: ## Upgrade implementation on a chain (ENV=dev CHAIN=sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make upgrade ENV=dev CHAIN=sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make upgrade ENV=dev CHAIN=sepolia"
	@exit 1
endif
	@./ops/upgrade.sh --env $(ENV) --chain $(CHAIN)

upgrade-verify: ## Upgrade with verification (ENV=dev CHAIN=sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make upgrade-verify ENV=dev CHAIN=sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make upgrade-verify ENV=dev CHAIN=sepolia"
	@exit 1
endif
	@./ops/upgrade.sh --env $(ENV) --chain $(CHAIN) --verify

upgrade-all: ## Upgrade implementation on all deployed chains (ENV=dev)
ifndef ENV
	@echo "Error: ENV is required. Usage: make upgrade-all ENV=dev"
	@exit 1
endif
	@./ops/upgrade.sh --env $(ENV) --all

upgrade-status: ## Show current deployment and implementation status
	@./ops/upgrade.sh --status

## Admin Commands

.PHONY: transfer-roles

transfer-roles: ## Transfer all roles to a new admin (ENV=prod CHAIN=base NEW_ADMIN=0x...)
ifndef ENV
	@echo "Error: ENV is required. Usage: make transfer-roles ENV=prod CHAIN=base NEW_ADMIN=0x..."
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required. Usage: make transfer-roles ENV=prod CHAIN=base NEW_ADMIN=0x..."
	@exit 1
endif
ifndef NEW_ADMIN
	@echo "Error: NEW_ADMIN is required. Usage: make transfer-roles ENV=prod CHAIN=base NEW_ADMIN=0x..."
	@exit 1
endif
	@./ops/transfer-roles.sh --env $(ENV) --chain $(CHAIN) --new-admin $(NEW_ADMIN)

## Status Commands

.PHONY: status

status: ## Show deployment status (alias for deploy-list)
	@./ops/deploy.sh --list

## Testing Commands

.PHONY: open-order fill-order cancel-order

open-order: ## Open a test order (ENV=dev CHAIN=sepolia TOKEN_IN=0x... AMOUNT_IN=... DEST_CHAIN=arbitrum_sepolia TOKEN_OUT=0x... AMOUNT_OUT=...)
ifndef ENV
	@echo "Error: ENV is required. Usage: make open-order ENV=dev CHAIN=sepolia TOKEN_IN=0x... AMOUNT_IN=1000000 DEST_CHAIN=arbitrum_sepolia TOKEN_OUT=0x... AMOUNT_OUT=1000000"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required."
	@exit 1
endif
ifndef TOKEN_IN
	@echo "Error: TOKEN_IN is required."
	@exit 1
endif
ifndef AMOUNT_IN
	@echo "Error: AMOUNT_IN is required."
	@exit 1
endif
ifndef DEST_CHAIN
	@echo "Error: DEST_CHAIN is required."
	@exit 1
endif
ifndef TOKEN_OUT
	@echo "Error: TOKEN_OUT is required."
	@exit 1
endif
ifndef AMOUNT_OUT
	@echo "Error: AMOUNT_OUT is required."
	@exit 1
endif
	@./ops/open-order.sh --env $(ENV) --chain $(CHAIN) \
		--token-in $(TOKEN_IN) --amount-in $(AMOUNT_IN) \
		--dest-chain $(DEST_CHAIN) --token-out $(TOKEN_OUT) --amount-out $(AMOUNT_OUT) \
		$(if $(RECIPIENT),--recipient $(RECIPIENT)) \
		$(if $(SOLVER),--solver $(SOLVER)) \
		$(if $(DEADLINE),--deadline $(DEADLINE))

fill-order: ## Fill a test order (ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... ORIGIN_CHAIN=sepolia AMOUNT_OUT=...)
ifndef ENV
	@echo "Error: ENV is required. Usage: make fill-order ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... ORIGIN_CHAIN=sepolia AMOUNT_OUT=1000000"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required."
	@exit 1
endif
ifndef ORDER_ID
	@echo "Error: ORDER_ID is required."
	@exit 1
endif
ifndef ORIGIN_CHAIN
	@echo "Error: ORIGIN_CHAIN is required."
	@exit 1
endif
ifndef AMOUNT_OUT
	@echo "Error: AMOUNT_OUT is required."
	@exit 1
endif
	@./ops/fill-order.sh --env $(ENV) --chain $(CHAIN) \
		--order-id $(ORDER_ID) --origin-chain $(ORIGIN_CHAIN) --amount-out $(AMOUNT_OUT) \
		$(if $(ORIGIN_RECIPIENT),--origin-recipient $(ORIGIN_RECIPIENT)) \
		$(if $(BRIDGE_ADAPTER_ARGS),--bridge-adapter-args $(BRIDGE_ADAPTER_ARGS))

cancel-order: ## Cancel a test order (ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... ORIGIN_CHAIN=sepolia)
ifndef ENV
	@echo "Error: ENV is required. Usage: make cancel-order ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... ORIGIN_CHAIN=sepolia"
	@exit 1
endif
ifndef CHAIN
	@echo "Error: CHAIN is required."
	@exit 1
endif
ifndef ORDER_ID
	@echo "Error: ORDER_ID is required."
	@exit 1
endif
ifndef ORIGIN_CHAIN
	@echo "Error: ORIGIN_CHAIN is required."
	@exit 1
endif
	@./ops/cancel-order.sh --env $(ENV) --chain $(CHAIN) \
		--order-id $(ORDER_ID) --origin-chain $(ORIGIN_CHAIN) \
		$(if $(BRIDGE_ADAPTER_ARGS),--bridge-adapter-args $(BRIDGE_ADAPTER_ARGS))

## Help

.PHONY: help

help: ## Show this help
	@echo "OrderBook Multi-Chain Deployment System"
	@echo ""
	@echo "All commands use 1Password CLI for secret management."
	@echo "Account: mzerolabs.1password.com"
	@echo ""
	@echo "Deployment:"
	@echo "  make deploy ENV=<dev|prod> CHAIN=<alias>         Deploy to a single chain"
	@echo "  make deploy-verify ENV=<dev|prod> CHAIN=<alias>  Deploy with verification"
	@echo "  make deploy-all ENV=<dev|prod>                   Deploy to all chains"
	@echo "  make deploy-list                                 List configured chains"
	@echo ""
	@echo "Configuration:"
	@echo "  make configure-routes ENV=<dev|prod>             Configure all routes"
	@echo "  make configure-dest ENV=<dev|prod> CHAIN=x DEST=y  Configure single route"
	@echo "  make verify-routes ENV=<dev|prod>                Verify on-chain config"
	@echo ""
	@echo "Upgrades:"
	@echo "  make upgrade ENV=<dev|prod> CHAIN=<alias>        Upgrade on a chain"
	@echo "  make upgrade-all ENV=<dev|prod>                  Upgrade on all chains"
	@echo "  make upgrade-status                              Show implementation status"
	@echo ""
	@echo "Admin:"
	@echo "  make transfer-roles ENV=<dev|prod> CHAIN=<alias> \\  Transfer all roles to new admin"
	@echo "       NEW_ADMIN=0x..."
	@echo ""
	@echo "Testing:"
	@echo "  make open-order ENV=<dev|prod> CHAIN=<alias> \\    Create a test order"
	@echo "       TOKEN_IN=0x... AMOUNT_IN=... \\"
	@echo "       DEST_CHAIN=<alias> TOKEN_OUT=0x... AMOUNT_OUT=..."
	@echo "  make fill-order ENV=<dev|prod> CHAIN=<alias> \\    Fill a test order"
	@echo "       ORDER_ID=0x... ORIGIN_CHAIN=<alias> AMOUNT_OUT=..."
	@echo "  make cancel-order ENV=<dev|prod> CHAIN=<alias> \\  Cancel a test order"
	@echo "       ORDER_ID=0x... ORIGIN_CHAIN=<alias>"
	@echo ""
	@echo "Status:"
	@echo "  make status                                      Show deployment status"
	@echo ""
	@echo "Options:"
	@echo "  DRY_RUN=true  Simulate without broadcasting (logs JSON to console)"
	@echo ""
	@echo "Environment files:"
	@echo "  .env.dev   - Development/testnet (Sepolia, Arbitrum Sepolia)"
	@echo "  .env.prod  - Production/mainnet"
	@echo ""
	@echo "Example workflow:"
	@echo "  1. make deploy ENV=dev CHAIN=sepolia"
	@echo "  2. make deploy ENV=dev CHAIN=arbitrum_sepolia"
	@echo "  3. make configure-routes ENV=dev"
	@echo "  4. make verify-routes ENV=dev"
	@echo ""
	@echo "Testing workflow:"
	@echo "  1. make open-order ENV=dev CHAIN=sepolia TOKEN_IN=0x... AMOUNT_IN=1e18 \\"
	@echo "       DEST_CHAIN=arbitrum_sepolia TOKEN_OUT=0x... AMOUNT_OUT=1e18"
	@echo "  2. make fill-order ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... \\"
	@echo "       ORIGIN_CHAIN=sepolia AMOUNT_OUT=1e18"
	@echo "  3. make cancel-order ENV=dev CHAIN=arbitrum_sepolia ORDER_ID=0x... \\"
	@echo "       ORIGIN_CHAIN=sepolia  (alternative to fill)"
	@echo ""
	@echo "Dry run example:"
	@echo "  make deploy ENV=dev CHAIN=sepolia DRY_RUN=true"

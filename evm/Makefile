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

## Status Commands

.PHONY: status

status: ## Show deployment status (alias for deploy-list)
	@./ops/deploy.sh --list

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
	@echo "Status:"
	@echo "  make status                                      Show deployment status"
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

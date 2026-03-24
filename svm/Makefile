# Default environment
env ?= devnet

# op run resolves op:// references in .env.svm into environment variables
# so secrets are only held in memory, never written to disk.
OP_RUN := op run --account mzerolabs.1password.com --env-file=.env.svm --no-masking --
INJECT := $(OP_RUN) envsubst < txtx.yml > raw.txtx.yml
CLEAN := rm raw.txtx.yml

# Surfpool Runbooks

deploy:
# 	anchor build -p order_book
	$(INJECT)
	surfpool run deployment -m raw.txtx.yml --env $(env)
	$(CLEAN)
	@echo "Deployment complete"

initialize:
	$(INJECT)
	surfpool run initialize -m raw.txtx.yml --env $(env)
	$(CLEAN)
	@echo "Initialization complete"

add-destination:
	$(INJECT)
	surfpool run add_destination -m raw.txtx.yml --env $(env)
	$(CLEAN)
	@echo "Destination added"

remove-destination:
	$(INJECT)
	surfpool run remove_destination -m raw.txtx.yml --env $(env) --unsupervised
	$(CLEAN)
	@echo "Destination removed"

run-localnet:
	$(INJECT)
	surfpool start -m raw.txtx.yml -r deployment -r initialize -a test4MzZzYk2NAP1222FSuKqq83GuXY5tHakqREDHPo
	$(CLEAN)

# TypeScript Scripts

open-order: ## Open a test order
	@./ops/open-order.sh --env $(env) \
		--token-in $(TOKEN_IN) --amount-in $(AMOUNT_IN) \
		--dest-chain-id $(DEST_CHAIN_ID) --token-out $(TOKEN_OUT) --amount-out $(AMOUNT_OUT) \
		$(if $(RECIPIENT),--recipient $(RECIPIENT)) \
		$(if $(SOLVER),--solver $(SOLVER)) \
		$(if $(DEADLINE),--deadline $(DEADLINE))
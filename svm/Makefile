# Default environment
env ?= devnet

# op run resolves op:// references in .env.svm into environment variables
# so secrets are only held in memory, never written to disk.
OP_RUN := op run --env-file=.env.svm --

# Surfpool Runbooks

deploy:
	anchor build -p order_book
	$(OP_RUN) surfpool run deployment --env $(env) --unsupervised
	@echo "Deployment complete"

initialize:
	$(OP_RUN) surfpool run initialize --env $(env) --unsupervised
	@echo "Initialization complete"

set-destination:
	$(OP_RUN) surfpool run add_destination --env $(env) --unsupervised
	@echo "Configuration complete"

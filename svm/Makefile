# Default environment
env ?= devnet

# Inject secrets from 1Password and generate txtx.yml
inject-secrets:
	@op inject -f -i txtx.yml.tpl -o txtx.yml
	@echo "Secrets injected into txtx.yml"

# Surfpool Runbooks

deploy: inject-secrets
	anchor build -p order_book
	surfpool run deployment --env $(env) --unsupervised
	@echo "Deployment complete"

initialize: inject-secrets
	surfpool run initialize --env $(env) --unsupervised
	@echo "Initialization complete"

set-destination: inject-secrets
	surfpool run add_destination --env $(env) --unsupervised
	@echo "Configuration complete"

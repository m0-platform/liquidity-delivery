run-solver:
	cd solver && cargo run --package solver --bin solver -- $(args)

run-svm-localnet:
	surfpool start -r deployment -r initialize -a test4MzZzYk2NAP1222FSuKqq83GuXY5tHakqREDHPo --rpc-url https://hatty-73mn84-fast-mainnet.helius-rpc.com

run-local-solver:
	docker compose -f docker-compose/docker-compose.yml up -d

restart-solver:
	docker compose -f docker-compose/docker-compose.yml up -d --build --no-deps solver

stop-local-solver:
	docker compose -f docker-compose/docker-compose.yml down

test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver -- --nocapture

# make parse-evm-error hex=0xc56873ba
parse-evm-error:
	@cast 4byte $(hex)

deploy-solver:
	railway environment devnet
	docker build --platform linux/amd64 -t ghcr.io/m0-foundation/liquidity-delivery:solver -f solver/Dockerfile .
	docker push ghcr.io/m0-foundation/liquidity-delivery:solver
	sleep 1
	railway redeploy --service Solver --yes

deploy-quoter:
	railway environment devnet
	docker build --platform linux/amd64 -t ghcr.io/m0-foundation/liquidity-delivery:quoter-service -f docker-compose/quoter/Dockerfile .
	docker push ghcr.io/m0-foundation/liquidity-delivery:quoter-service
	sleep 1
	railway redeploy --service Quoter --yes

deploy-orderbook-devnet:
	anchor build -p order_book
	surfpool run deployment --env devnet --unsupervised

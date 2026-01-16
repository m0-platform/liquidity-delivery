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
	RUSTFLAGS='-Awarnings' cargo test --package solver --test integration_test -- --nocapture

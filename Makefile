run-solver:
	cd solver && cargo run --package solver --bin solver -- $(args)

run-svm-localnet:
	surfpool start -r deployment -r initialize -a test4MzZzYk2NAP1222FSuKqq83GuXY5tHakqREDHPo --rpc-url https://hatty-73mn84-fast-mainnet.helius-rpc.com

run-local-solver:
	docker compose up -d && docker compose logs -f solver

test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver --test integration_test -- --nocapture

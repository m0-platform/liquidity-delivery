run-solver:
	cd solver && cargo run --package solver --bin solver -- $(args)

run-svm-localnet:
	surfpool start -r deployment -r initialize -a test4MzZzYk2NAP1222FSuKqq83GuXY5tHakqREDHPo --rpc-url https://hatty-73mn84-fast-mainnet.helius-rpc.com

test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver -- --nocapture

# make parse-evm-error hex=0xc56873ba
parse-evm-error:
	@cast 4byte $(hex)

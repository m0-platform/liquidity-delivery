run-solver:
	cd solver && cargo run --package solver --bin solver -- $(args)

test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver -- --nocapture

# make parse-evm-error hex=0xc56873ba
parse-evm-error:
	@cast 4byte $(hex)

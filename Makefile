test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver --test integration_test -- --nocapture

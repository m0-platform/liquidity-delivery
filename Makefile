test-solver:
	RUSTFLAGS='-Awarnings' cargo test --package solver --test integration_tests -- --nocapture

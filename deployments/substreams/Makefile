build-and-publish-solana-devnet:
	substreams build --manifest substreams-solana-devnet.yaml
	substreams build solana-devnet-orderbook-v0.1.0.spkg

build-and-publish-sepolia:
	substreams build --manifest substreams-sepolia.yaml
	substreams build sepolia-orderbook-v0.1.0.spkg

build-docker-image:
	docker build --platform linux/amd64 -t ghcr.io/m0-foundation/liquidity-delivery:orderbook-substreams .
	docker push ghcr.io/m0-foundation/liquidity-delivery:orderbook-substreams

debug-solana-gui:
	substreams gui substreams-solana-devnet.yaml --stop-block +10 --start-block 439866040

debug-sepolia-gui:
	substreams gui substreams-sepolia.yaml --stop-block +10 --start-block 10190942

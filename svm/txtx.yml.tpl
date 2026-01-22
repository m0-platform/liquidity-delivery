---
name: liquidity-delivery
id: liquidity-delivery
runbooks:
  - name: deployment
    description: Deploy programs
    location: runbooks/deployment
  - name: initialize
    description: Initialize programs
    location: runbooks/initialize
  - name: add_destination
    description: Configure programs
    location: runbooks/add_destination
environments:
  localnet:
    network_id: localnet
    rpc_api_url: http://127.0.0.1:8899
    chain_id: 1399811149
    local_signer: 43fc825e966dfb386828d229d27f67d1fc9d2906d2f76903fde9e8a2a17e88460d3b72feead9928540668b0cc280f48ee1ecf91230b1cfae78190f21390d0e9a
  devnet:
    network_id: devnet
    chain_id: 1399811150
    rpc_api_url: "op://Solana Dev/Helius/dev rpc"
  mainnet:
    network_id: mainnet
    rpc_api_url: "op://Solana Dev/Helius/prod rpc"
    chain_id: 1399811149
    build_dir: target/verifiable

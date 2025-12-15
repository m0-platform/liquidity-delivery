use alloy::{
    hex,
    network::TransactionBuilder,
    node_bindings::AnvilInstance,
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use anchor_client::solana_sdk::signature::Keypair;
use mockito::ServerGuard;
use regex::Regex;
use slog::{info, Drain, Logger};
use solver::{
    common_logger_values,
    providers::Signers,
    utils::{chain_from_id, decode_evm_address},
    Config,
};
use std::{process::Command, sync::Arc, thread::sleep, time::Duration};
use test_context::AsyncTestContext;
use tokio::sync::broadcast;

use crate::common::{mock_api, Asset, LogBuffer};

sol!(
    #[sol(rpc)]
    IOrderBook,
    "../evm/out/OrderBook.sol/OrderBook.json"
);

sol! {
    #[sol(rpc)]
    interface MockERC20 {
        function mint(address to, uint256 amount) external;
        function balanceOf(address account) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

pub use IOrderBook::OrderParams;

pub struct TestSuite {
    pub chains: Vec<ChainInstance>,
    _evm_signer: PrivateKeySigner,
    pub evm_user: PrivateKeySigner,
    _svm_signer: Arc<Keypair>,
    pub shutdown_tx: broadcast::Sender<()>,
    _mock_server: ServerGuard,
    log_buffer: LogBuffer,
    pub logger: Logger,
}

pub struct ChainInstance {
    pub anvil: AnvilInstance,
    pub chain_id: u32,
    pub contract_address: Address,
    pub tokens: Vec<Asset>,
}

impl AsyncTestContext for TestSuite {
    /// Create a new test suite with Anvil and deployed contracts
    async fn setup() -> TestSuite {
        // Create a log buffer for capturing logs
        let log_buffer = LogBuffer::new();
        let logger = Logger::root(
            slog_async::Async::new(log_buffer.clone().fuse())
                .build()
                .fuse(),
            common_logger_values!(),
        );

        let evm_chains = vec![1, 8453];

        let mut chains = Vec::new();
        let evm_signer = PrivateKeySigner::from_bytes(&FixedBytes::from([1u8; 32])).unwrap();
        let evm_user = PrivateKeySigner::from_bytes(&FixedBytes::from([2u8; 32])).unwrap();
        let svm_signer = Arc::new(Keypair::new());

        // Start Anvil nodes for each chain
        for (i, &chain_id) in evm_chains.iter().enumerate() {
            let anvil = alloy::node_bindings::Anvil::new()
                .block_time_f64(0.1)
                .chain_id(chain_id as u64)
                .try_spawn()
                .expect("failed to spawn anvil node");

            // Send ETH from funded account to our signer and user
            for address in [evm_signer.address(), evm_user.address()] {
                let anvil_wallet = anvil.wallet().expect("expected anvil wallet");

                let tx = TransactionRequest::default()
                    .with_from(anvil_wallet.default_signer().address())
                    .with_to(address)
                    .with_value(U256::from(10).pow(U256::from(18)));

                ProviderBuilder::new()
                    .wallet(anvil_wallet)
                    .connect_http(anvil.endpoint_url())
                    .send_transaction(tx)
                    .await
                    .expect("failed to send eth")
                    .watch()
                    .await
                    .expect("failed to confirm tx");
            }

            // Provider with our crosschain signer
            let provider = ProviderBuilder::new()
                .wallet(evm_signer.clone())
                .connect_http(anvil.endpoint_url());

            let contract = IOrderBook::deploy(provider.clone(), chain_id, Address::new([0u8; 20]))
                .await
                .expect("Failed to deploy contract");

            let &contract_address = contract.address();

            // Initialize the contract with admin role
            contract
                .initialize(evm_signer.address())
                .send()
                .await
                .expect("Failed to send initialize transaction")
                .get_receipt()
                .await
                .expect("Failed to confirm initialize transaction");

            // Set destination config
            let &dest_chain = evm_chains.get((i + 1) % evm_chains.len()).unwrap_or(&8453);
            contract
                .setDestinationConfig(dest_chain, true, 10)
                .send()
                .await
                .expect("Failed to send setDestinationConfig transaction for chain 1")
                .get_receipt()
                .await
                .expect("Failed to confirm setDestinationConfig transaction for chain 1");

            // Deploy mock tokens for testing
            let mut tokens = Vec::new();

            // Deploy 3 mock tokens: USDC, USDT, USDS
            for i in 0..3 {
                let bytecode_hex = "608060405234801561000f575f80fd5b50604051610a16380380610a1683398101604081905261002e91610100565b5f6100398482610201565b5060016100468382610201565b506002805460ff191660ff92909216919091179055506102bb9050565b634e487b7160e01b5f52604160045260245ffd5b5f82601f830112610086575f80fd5b81516001600160401b0381111561009f5761009f610063565b604051601f8201601f19908116603f011681016001600160401b03811182821017156100cd576100cd610063565b6040528181528382016020018510156100e4575f80fd5b8160208501602083015e5f918101602001919091529392505050565b5f805f60608486031215610112575f80fd5b83516001600160401b03811115610127575f80fd5b61013386828701610077565b602086015190945090506001600160401b03811115610150575f80fd5b61015c86828701610077565b925050604084015160ff81168114610172575f80fd5b809150509250925092565b600181811c9082168061019157607f821691505b6020821081036101af57634e487b7160e01b5f52602260045260245ffd5b50919050565b601f8211156101fc57805f5260205f20601f840160051c810160208510156101da5750805b601f840160051c820191505b818110156101f9575f81556001016101e6565b50505b505050565b81516001600160401b0381111561021a5761021a610063565b61022e81610228845461017d565b846101b5565b6020601f821160018114610260575f83156102495750848201515b5f19600385901b1c1916600184901b1784556101f9565b5f84815260208120601f198516915b8281101561028f578785015182556020948501946001909201910161026f565b50848210156102ac57868401515f19600387901b60f8161c191681555b50505050600190811b01905550565b61074e806102c85f395ff3fe608060405234801561000f575f80fd5b50600436106100b9575f3560e01c806340c10f191161007257806395d89b411161005857806395d89b411461017b578063a9059cbb14610183578063dd62ed3e14610196575f80fd5b806340c10f191461014757806370a082311461015c575f80fd5b806318160ddd116100a257806318160ddd146100fe57806323b872dd14610115578063313ce56714610128575f80fd5b806306fdde03146100bd578063095ea7b3146100db575b5f80fd5b6100c56101c0565b6040516100d29190610546565b60405180910390f35b6100ee6100e93660046105c1565b61024b565b60405190151581526020016100d2565b61010760035481565b6040519081526020016100d2565b6100ee6101233660046105e9565b6102c4565b6002546101359060ff1681565b60405160ff90911681526020016100d2565b61015a6101553660046105c1565b6103ef565b005b61010761016a366004610623565b60046020525f908152604090205481565b6100c5610491565b6100ee6101913660046105c1565b61049e565b6101076101a4366004610643565b600560209081525f928352604080842090915290825290205481565b5f80546101cc90610674565b80601f01602080910402602001604051908101604052809291908181526020018280546101f890610674565b80156102435780601f1061021a57610100808354040283529160200191610243565b820191905f5260205f20905b81548152906001019060200180831161022657829003601f168201915b505050505081565b335f81815260056020908152604080832073ffffffffffffffffffffffffffffffffffffffff8716808552925280832085905551919290917f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925906102b29086815260200190565b60405180910390a35060015b92915050565b73ffffffffffffffffffffffffffffffffffffffff83165f9081526005602090815260408083203384529091528120805483919083906103059084906106f2565b909155505073ffffffffffffffffffffffffffffffffffffffff84165f908152600460205260408120805484929061033e9084906106f2565b909155505073ffffffffffffffffffffffffffffffffffffffff83165f9081526004602052604081208054849290610377908490610705565b925050819055508273ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef846040516103dd91815260200190565b60405180910390a35060019392505050565b73ffffffffffffffffffffffffffffffffffffffff82165f9081526004602052604081208054839290610423908490610705565b925050819055508060035f82825461043b9190610705565b909155505060405181815273ffffffffffffffffffffffffffffffffffffffff8316905f907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9060200160405180910390a35050565b600180546101cc90610674565b335f908152600460205260408120805483919083906104be9084906106f2565b909155505073ffffffffffffffffffffffffffffffffffffffff83165f90815260046020526040812080548492906104f7908490610705565b909155505060405182815273ffffffffffffffffffffffffffffffffffffffff84169033907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef906020016102b2565b602081525f82518060208401528060208501604085015e5f6040828501015260407fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe0601f83011684010191505092915050565b803573ffffffffffffffffffffffffffffffffffffffff811681146105bc575f80fd5b919050565b5f80604083850312156105d2575f80fd5b6105db83610599565b946020939093013593505050565b5f805f606084860312156105fb575f80fd5b61060484610599565b925061061260208501610599565b929592945050506040919091013590565b5f60208284031215610633575f80fd5b61063c82610599565b9392505050565b5f8060408385031215610654575f80fd5b61065d83610599565b915061066b60208401610599565b90509250929050565b600181811c9082168061068857607f821691505b6020821081036106bf577f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b50919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b818103818111156102be576102be6106c5565b808201808211156102be576102be6106c556fea2646970667358221220080bc754c71e020f26b0f59c95cda3565043cff69f037aeb7276cb162d21bb4964736f6c634300081a0033";
                let constructor_args_hex = "000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000a5465737420546f6b656e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000045445535400000000000000000000000000000000000000000000000000000000";

                let bytecode = hex::decode(format!("{}{}", bytecode_hex, constructor_args_hex))
                    .expect("Failed to decode bytecode");

                let tx = TransactionRequest::default().with_deploy_code(bytecode);

                // Deploy ERC20 contract
                let receipt = provider
                    .send_transaction(tx)
                    .await
                    .expect("Failed to send deployment transaction")
                    .get_receipt()
                    .await
                    .expect("Failed to get deployment receipt");

                let token_address = receipt
                    .contract_address
                    .expect("Failed to get deployed contract address");

                let token = MockERC20::new(token_address, provider.clone());

                // Mint 100 tokens to the signer and user
                let amount = U256::from(100) * U256::from(10).pow(U256::from(6));

                for address in [evm_signer.address(), evm_user.address()] {
                    token
                        .mint(address, amount)
                        .send()
                        .await
                        .expect("Failed to send mint transaction")
                        .get_receipt()
                        .await
                        .expect("Failed to confirm mint transaction");
                }

                tokens.push(Asset {
                    address: token_address,
                    chain_id,
                    symbol: ["USDC", "USDT", "USDS"][i].to_string(),
                });

                // User need to approve spends
                let signer_token = MockERC20::new(
                    token_address,
                    ProviderBuilder::new()
                        .wallet(evm_user.clone())
                        .connect_http(anvil.endpoint_url()),
                );

                signer_token
                    .approve(contract_address, U256::MAX)
                    .send()
                    .await
                    .expect("Failed to send approve transaction")
                    .get_receipt()
                    .await
                    .expect("Failed to confirm approve transaction");
            }

            let instance = ChainInstance {
                anvil,
                chain_id,
                contract_address,
                tokens,
            };

            chains.push(instance);
        }

        // Create mock API with the test tokens
        let api_tokens = chains
            .iter()
            .flat_map(|chain| chain.tokens.iter().cloned())
            .collect::<Vec<Asset>>();

        let mock_server = mock_api::mock_api_with_assets(api_tokens).await;

        // Setup solver
        let mut config = Config::default();

        // Anvil chain configurations
        for chain in &chains {
            config.chains.push(solver::config::ChainConfig {
                chain_id: chain.chain_id,
                chain: chain_from_id(chain.chain_id),
                rpc_url: chain.anvil.endpoint_url().to_string(),
                ws_url: chain.anvil.ws_endpoint_url().to_string(),
                order_book_address: chain.contract_address.to_string(),
            });
        }

        config.liquidity_api_url = mock_server.url();
        config.signers = Signers::new(evm_signer.clone(), svm_signer.clone());

        let shutdown_tx = solver::run_solver(config, logger.clone())
            .await
            .expect("Failed to start solver");

        let suite = TestSuite {
            chains,
            _evm_signer: evm_signer,
            evm_user,
            _svm_signer: svm_signer,
            shutdown_tx,
            _mock_server: mock_server,
            log_buffer,
            logger: logger.new(slog::o!("component" => "TestSuite")),
        };

        // Wait for solver to start
        suite.contains_log("Started event listener for chain");

        suite
    }

    async fn teardown(self) {
        let _ = self.shutdown_tx.send(());

        for chain in self.chains {
            let id = chain.anvil.child().id();
            let _ = Command::new("kill").arg("-9").arg(id.to_string()).output();
        }
    }
}

impl TestSuite {
    pub fn contains_log(&self, pattern: &str) {
        let timeout = Duration::from_secs(5);
        let poll_interval = Duration::from_millis(50);
        let start = std::time::Instant::now();

        let re = Regex::new(&format!(".*{}.*", pattern)).unwrap();

        while start.elapsed() < timeout {
            if re.is_match(&self.log_buffer.to_string()) {
                return;
            }
            sleep(poll_interval);
        }

        panic!(
            "Missing expected log pattern: {}\n\n=== LOGS ===\n{}\n",
            pattern,
            self.log_buffer.to_string()
        );
    }

    pub fn contains_order_lifecycle(&self, order_id: &str, events: &[&str]) {
        for &event in events {
            let pattern = format!("{} .* order_id={}", event, order_id);
            self.contains_log(&pattern);
        }
    }

    pub async fn create_order(
        &self,
        chain: &ChainInstance,
        token_in: Address,
        token_out: Address,
        dest_chain_id: u32,
        amount_in: u128,
        amount_out: u128,
    ) {
        let provider = ProviderBuilder::new()
            .wallet(self.evm_user.clone())
            .connect_http(chain.anvil.endpoint_url());

        let contract = IOrderBook::new(chain.contract_address, provider);

        let builder = contract.openOrder(OrderParams {
            tokenIn: token_in,
            destChainId: dest_chain_id,
            tokenOut: decode_evm_address(token_out).into(),
            amountIn: amount_in,
            amountOut: amount_out,
            recipient: decode_evm_address(self.evm_user.address()).into(),
            fillDeadline: u32::MAX,
            solver: [0u8; 32].into(),
        });

        let receipt = builder
            .send()
            .await
            .expect("Failed to send openOrder transaction")
            .get_receipt()
            .await
            .expect("Failed to confirm mint transaction");

        info!(self.logger, "Created order on chain {}", chain.chain_id;
            "block_number" => receipt.block_number.unwrap_or(0)
        );
    }
}

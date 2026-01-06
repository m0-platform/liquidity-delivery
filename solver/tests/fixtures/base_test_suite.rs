#![allow(dead_code)]

use alloy::{
    network::TransactionBuilder,
    node_bindings::AnvilInstance,
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::{k256::sha2::digest::Key, local::PrivateKeySigner},
    sol,
};
use anchor_client::{
    solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, system_program},
    Client, Cluster,
};
use mockito::ServerGuard;
use regex::Regex;
use serde_json::json;
use slog::{info, Drain, Logger};
use solana_client::nonblocking::rpc_client::RpcClient;
use solver::{
    config::{Environment, SupportedAssets},
    providers::Signers,
    utils::{chain_from_id, decode_address, decode_evm_address},
    Config,
};
use std::{
    str::FromStr,
    sync::Arc,
    time::{self, Duration},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{self, Command},
    sync::broadcast,
    time::sleep,
};

use crate::common::{create_and_mint_token, create_open_order, orderbook_pda, svm};
use crate::common::{mock_api, Asset, LogBuffer};

sol!(
    #[sol(rpc)]
    IOrderBook,
    "../evm/out/OrderBook.sol/OrderBook.json"
);

sol!(
    #[sol(rpc)]
    exampleERC20,
    "tests/artifacts/exampleERC20.json"
);

pub use IOrderBook::OrderParams;

pub struct BaseTestSuite {
    pub chains: Vec<ChainInstance>,
    _evm_signer: PrivateKeySigner,
    evm_user: PrivateKeySigner,
    _svm_signer: Arc<Keypair>,
    svm_user: Arc<Keypair>,
    pub svm_mint: Option<Pubkey>,
    pub shutdown_tx: broadcast::Sender<()>,
    _mock_server: ServerGuard,
    pub log_buffer: LogBuffer,
    logger: Logger,
    quoter_process: Option<ProcessWithPort>,
    surfpool_process: Option<ProcessWithPort>,
}

pub struct ProcessWithPort {
    pub process: process::Child,
    pub port: u16,
}

pub struct ChainInstance {
    pub anvil: AnvilInstance,
    pub chain_id: u32,
    pub contract_address: Address,
    pub tokens: Vec<Asset>,
}

impl BaseTestSuite {
    /// Create a new test suite with Anvil and deployed contracts
    pub async fn setup_with_chains(evm_chains: Vec<u32>, start_quoter_api: bool) -> BaseTestSuite {
        // Create a log buffer for capturing logs
        let log_buffer = LogBuffer::new();
        let logger = Logger::root(
            slog_async::Async::new(log_buffer.clone().fuse())
                .build()
                .fuse(),
            slog::o!("component" => "TestSuite"),
        );

        let mut chains = Vec::new();
        let evm_signer = PrivateKeySigner::from_bytes(&FixedBytes::from([1u8; 32])).unwrap();
        let evm_user = PrivateKeySigner::from_bytes(&FixedBytes::from([2u8; 32])).unwrap();
        let svm_signer = Arc::new(Keypair::from_base58_string("2MqZwxzsfaEvQvnj4CgvUo2aknYXxJW2bBn5ewbftnbjU9DAtWX1XzCHy7Wd8dBSq5bmRwj6Ya5XTAnEe8sy2qS9"));
        let svm_user = Arc::new(Keypair::new());
        let mut surfpool_process = None;
        let mut svm_mint = None;

        // Start Anvil nodes for each chain (or Surfpool for Solana)
        for (i, &chain_id) in evm_chains.iter().enumerate() {
            // Solana
            if chain_id == 1399811149 {
                let port = portpicker::pick_unused_port().expect("No free ports available");

                let child = Command::new("surfpool")
                    .args(&[
                        "start",
                        "--port",
                        port.to_string().as_str(),
                        "--no-deploy",
                        "--no-tui",
                        "--airdrop",
                        &svm_signer.pubkey().to_string(),
                        "--airdrop",
                        &svm_user.pubkey().to_string(),
                        "--rpc-url",
                        "https://hatty-73mn84-fast-mainnet.helius-rpc.com",
                    ])
                    .current_dir("..")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to start surfpool");

                sleep(Duration::from_millis(1000)).await;

                for runbook in &["deployment", "initialize", "configure"] {
                    Command::new("surfpool")
                        .args(&[
                            "run",
                            runbook,
                            "--env",
                            "localnet",
                            "--unsupervised",
                            "--input",
                            format!("rpc_api_url=http://127.0.0.1:{}", port).as_str(),
                        ])
                        .current_dir("..")
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .output()
                        .await
                        .expect("failed to run surfpool cmd");
                }

                let client = RpcClient::new(format!("http://localhost:{}", port));

                client
                    .request_airdrop(&svm_user.pubkey(), 2_000_000_000)
                    .await
                    .expect("failed to airdrop to svm user");

                // Create and mint test token to svm_user
                svm_mint = Some(
                    create_and_mint_token(client, &svm_signer, &svm_user.pubkey(), 100000000).await,
                );

                surfpool_process = Some(ProcessWithPort {
                    process: child,
                    port,
                });

                continue;
            }

            let anvil = alloy::node_bindings::Anvil::new()
                .block_time_f64(0.1)
                .arg("--prune-history")
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
                let token = exampleERC20::deploy(&provider)
                    .await
                    .expect("erc20 token deploy failed");

                let amount = if i == 2 {
                    // Only leave solver with 10 tokens to test insufficient funds
                    U256::from(999999999999990u128) * U256::from(10).pow(U256::from(6))
                } else {
                    U256::from(100) * U256::from(10).pow(U256::from(6))
                };

                // Transfer 100 tokens to the user
                token
                    .transfer(evm_user.address(), amount)
                    .send()
                    .await
                    .expect("Failed to send mint transaction")
                    .get_receipt()
                    .await
                    .expect("Failed to confirm mint transaction");

                tokens.push(Asset {
                    address: token.address().to_string(),
                    chain_id,
                    symbol: ["USDC", "USDT", "USDS"][i].to_string(),
                });

                // User need to approve spends
                let signer_token = exampleERC20::new(
                    *token.address(),
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
        let mut api_tokens = chains
            .iter()
            .flat_map(|chain| chain.tokens.iter().cloned())
            .collect::<Vec<Asset>>();

        if let Some(mint) = &svm_mint {
            api_tokens.push(Asset {
                address: mint.to_string(),
                chain_id: 1399811149,
                symbol: "wM".to_string(),
            });
        }

        let mock_server = mock_api::mock_api_with_assets(api_tokens).await;

        // Setup solver
        let mut config = Config::default();
        config.environment = Environment::Local;

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

        if let Some(surfpool_process) = &surfpool_process {
            config.chains.push(solver::config::ChainConfig {
                chain_id: 1399811149,
                chain: chain_from_id(1399811149),
                rpc_url: format!("http://localhost:{}", surfpool_process.port),
                ws_url: format!("ws://localhost:{}", surfpool_process.port),
                order_book_address: "MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK".to_string(),
            });
        }

        config.liquidity_api_url = mock_server.url();
        config.signers = Signers::new(evm_signer.clone(), svm_signer.clone());
        config.max_order_clip_size = 100;
        config.max_clip_reprocess_delay_sec = 1;
        config.connect_to_quote_stream = start_quoter_api;

        // Start quoter API if requested
        let quoter_process = if start_quoter_api {
            let port = portpicker::pick_unused_port().expect("No free ports available");
            let grpc_port = portpicker::pick_unused_port().expect("No free ports available");

            let child = Command::new("cargo")
                .args(["run", "--bin", "quoter"])
                .current_dir("../quoter")
                .env("API_PORT", port.to_string())
                .env("GRPC_PORT", grpc_port.to_string())
                .env("QUOTE_TIMEOUT_MS", "1500")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("Failed to start quoter process");

            // Poll quoter health endpoint until it's ready
            let health_url = format!("http://localhost:{}/health", port);
            let start_time = std::time::Instant::now();

            loop {
                if start_time.elapsed() > Duration::from_secs(60) {
                    panic!("Quoter failed to start");
                }

                if let Ok(response) = reqwest::get(&health_url).await {
                    if response.status().is_success() {
                        break;
                    }
                }

                sleep(Duration::from_millis(100)).await;
            }

            config.quoter_grpc_url = format!("http://localhost:{}", grpc_port);

            Some(ProcessWithPort {
                process: child,
                port,
            })
        } else {
            None
        };

        // Support created assets
        config.supported_assets = SupportedAssets {
            third_party_whitelist: chains
                .iter()
                .flat_map(|chain| {
                    chain
                        .tokens
                        .iter()
                        .map(|token| token.address.to_string())
                        .collect::<Vec<String>>()
                })
                .collect(),
            first_party_blacklist: vec![],
        };

        let shutdown_tx = solver::run_solver(config, logger.clone())
            .await
            .expect("Failed to start solver");

        let suite = BaseTestSuite {
            chains,
            _evm_signer: evm_signer,
            evm_user,
            _svm_signer: svm_signer,
            svm_user,
            svm_mint,
            surfpool_process,
            quoter_process,
            shutdown_tx,
            _mock_server: mock_server,
            log_buffer,
            logger,
        };

        // Wait for solver to start
        suite.contains_log("All components registered").await;

        suite
    }

    pub async fn base_teardown(mut self) {
        let _ = self.shutdown_tx.send(());

        for chain in self.chains.iter() {
            let id = chain.anvil.child().id();
            let _ = Command::new("kill").arg("-9").arg(id.to_string()).output();
        }

        if let Some(mut quoter) = self.quoter_process.take() {
            let _ = quoter.process.kill();
        }
    }

    pub async fn contains_log(&self, pattern: &str) -> usize {
        self.contains_log_from_index(pattern, 0).await
    }

    pub async fn contains_log_from_index(&self, pattern: &str, start_index: usize) -> usize {
        let timeout = Duration::from_secs(10);
        let poll_interval = Duration::from_millis(50);
        let start = std::time::Instant::now();

        let re = Regex::new(&format!(".*{}.*", pattern)).unwrap();

        while start.elapsed() < timeout {
            let logs = self.log_buffer.to_string().split_off(start_index);

            if let Some(mat) = re.find(&logs) {
                return start_index + mat.end();
            }

            // Fail early
            if logs.contains("Error handling event") {
                break;
            }

            sleep(poll_interval).await;
        }

        println!("\n=== LOGS ===\n{}", self.log_buffer.to_string());

        panic!(
            "Missing expected log pattern: {}\n{}",
            pattern,
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        );
    }

    pub async fn contains_order_lifecycle(&self, order_id: &str, events: &[&str]) {
        let mut start_index: usize = 0;
        for &event in events {
            let pattern = format!("{} .* order_id={}", event, order_id);
            start_index = self.contains_log_from_index(&pattern, start_index).await;
        }
    }

    pub async fn create_order(
        &self,
        chain: &ChainInstance,
        token_in: String,
        token_out: String,
        dest_chain_id: u32,
        amount_in: u128,
        amount_out: u128,
    ) {
        let provider = ProviderBuilder::new()
            .wallet(self.evm_user.clone())
            .connect_http(chain.anvil.endpoint_url());

        let contract = IOrderBook::new(chain.contract_address, provider);

        let builder = contract.openOrder(OrderParams {
            tokenIn: token_in.parse().unwrap(),
            destChainId: dest_chain_id,
            tokenOut: decode_address(token_out, dest_chain_id).unwrap().into(),
            amountIn: amount_in,
            amountOut: amount_out,
            recipient: decode_evm_address(self.evm_user.address()).into(),
            fillDeadline: u32::MAX,
            solver: [0u8; 32].into(),
        });

        builder
            .send()
            .await
            .expect("Failed to send openOrder transaction")
            .get_receipt()
            .await
            .expect("Failed to confirm mint transaction");
    }

    pub async fn create_svm_order(
        &self,
        token_in: &Pubkey,
        token_out: String,
        dest_chain_id: u32,
        amount_in: u64,
        amount_out: u64,
    ) {
        let client = Client::new(
            Cluster::from_str(&self.surfpool_endpoint()).unwrap(),
            self.svm_user.clone(),
        );

        let program = client.program(order_book::ID).unwrap();

        create_open_order(
            program,
            token_in,
            &order_book::instructions::open::OrderParams {
                token_out: decode_address(token_out, dest_chain_id).unwrap(),
                dest_chain_id,
                amount_in,
                amount_out: amount_out as u128,
                recipient: [0u8; 32],
                fill_deadline: u64::MAX,
                solver: [0u8; 32].into(),
            },
        )
        .await;
    }

    pub fn quote_endpoint(&self) -> String {
        format!(
            "http://localhost:{}/quote",
            self.quoter_process.as_ref().expect("Quoter not set").port
        )
    }

    pub fn surfpool_endpoint(&self) -> String {
        format!(
            "http://localhost:{}",
            self.surfpool_process
                .as_ref()
                .expect("Surfpool not set")
                .port
        )
    }
}

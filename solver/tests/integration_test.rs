mod mock_api;
mod tracing_capture;

use alloy::{
    hex,
    network::TransactionBuilder,
    node_bindings::{Anvil, AnvilInstance},
    primitives::{Address, FixedBytes, U256},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, Provider, ProviderBuilder, RootProvider,
    },
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use anchor_client::solana_sdk::signature::Keypair;
use m0_liquidity_sdk::types::Chain;
use solver::{config::Signers, utils::decode_evm_address, Config};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, OnceCell},
    time::sleep,
};

use crate::{mock_api::AssetConfig, IOrderBook::OrderParams};

sol!(
    #[sol(rpc)]
    OrderBook,
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

struct TestSuite {
    anvil: AnvilInstance,
    contract_address: Address,
    tokens: Vec<Address>,
    evm_signer: PrivateKeySigner,
    svm_signer: Arc<Keypair>,
    shutdown_tx: broadcast::Sender<()>,
    log_capture: tracing_capture::CaptureLayer,
}

impl TestSuite {
    async fn init() -> Self {
        let log_capture = tracing_capture::get_capture();

        // Start Anvil node
        let anvil = Anvil::new()
            .block_time(1)
            .chain_id(11155111)
            .try_spawn()
            .expect("failed to spawn anvil node");

        let evm_signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let svm_signer = Arc::new(Keypair::new());

        let provider = ProviderBuilder::new()
            .wallet(evm_signer.clone())
            .connect_http(anvil.endpoint_url());

        let contract = OrderBook::deploy(&provider, 11155111, Address::new([0u8; 20]))
            .await
            .expect("Failed to deploy contract");

        let contract_address = *contract.address();

        // Deploy mock tokens for testing
        let mut tokens = Vec::new();
        let mut api_tokens = Vec::new();

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

            // Create MockERC20 instance for interacting with the deployed contract
            let token = MockERC20::new(token_address, &provider);

            // Mint 100 tokens to the signer
            token
                .mint(
                    evm_signer.address(),
                    U256::from(100) * U256::from(10).pow(U256::from(6)),
                )
                .send()
                .await
                .expect("Failed to send mint transaction")
                .get_receipt()
                .await
                .expect("Failed to confirm mint transaction");

            tokens.push(token_address);
            api_tokens.push(AssetConfig::new(
                token_address.to_string(),
                "Sepolia",
                ["USDC", "USDT", "USDS"][i],
            ));

            // Approve the OrderBook contract to spend tokens
            let token = MockERC20::new(token_address, &provider);
            token
                .approve(contract_address, U256::MAX)
                .send()
                .await
                .expect("Failed to send approve transaction")
                .get_receipt()
                .await
                .expect("Failed to confirm approve transaction");
        }

        // Create mock API with the test tokens
        let mock_server = mock_api::mock_api_with_assets(api_tokens).await;

        // Setup solver
        let shutdown_tx = {
            let mut config = Config::default();

            // Anvil chain configuration
            config.chains.push(solver::config::ChainConfig {
                chain_id: 11155111,
                chain: Chain::Sepolia,
                rpc_url: anvil.endpoint_url().to_string(),
                ws_url: anvil.ws_endpoint_url().to_string(),
                order_book_address: contract_address.to_string(),
            });

            config.liquidity_api_url = mock_server.url();
            config.signers = Signers::new(evm_signer.clone(), svm_signer.clone());

            let shutdown_tx = solver::run_solver(config)
                .await
                .expect("Failed to start solver");

            // Let the solver boot up
            sleep(Duration::from_millis(10)).await;

            shutdown_tx
        };

        Self {
            anvil,
            contract_address,
            tokens,
            evm_signer,
            svm_signer,
            shutdown_tx,
            log_capture,
        }
    }
}

// Singleton instance of the shared test infrastructure
static SHARED_INFRA: OnceCell<TestSuite> = OnceCell::const_new();

// Get or initialize the shared test infrastructure
async fn get_tests_suite() -> &'static TestSuite {
    SHARED_INFRA
        .get_or_init(|| async { TestSuite::init().await })
        .await
}

#[tokio::test]
async fn test_inventory_manager_loads_balances() {
    let suite = get_tests_suite().await;

    assert!(suite.log_capture.contains("USDC: 100"));
    assert!(suite.log_capture.contains("USDT: 100"));
    assert!(suite.log_capture.contains("USDS: 100"));
    assert!(suite.log_capture.contains("ETH: 9999.99"));
}

#[tokio::test]
async fn test_order_rejected() {
    let suite = get_tests_suite().await;

    let provider = ProviderBuilder::new()
        .wallet(suite.evm_signer.clone())
        .connect_http(suite.anvil.endpoint_url());

    let contract = OrderBook::new(suite.contract_address, &provider);

    let builder = contract.openOrder(OrderParams {
        tokenIn: suite.tokens[0].into(),
        destChainId: 11155111,
        // Unsupported token
        tokenOut: FixedBytes::from([0u8; 32]),
        amountIn: 1000000,
        amountOut: 1000000,
        recipient: FixedBytes::from(decode_evm_address(suite.evm_signer.address())),
        fillDeadline: u32::MAX,
        solver: FixedBytes::from([0u8; 32]),
    });

    builder
        .send()
        .await
        .expect("Failed to send openOrder transaction")
        .get_receipt()
        .await
        .expect("Failed to confirm transaction");

    // Wait for the solver to process the order
    let rejected = wait_for_log(&suite.log_capture, "event=\"OrderRejected\" order_id=3cc8eacf0fb4494f90d52f2fd566e3750b21b8b88f86b9fdce50b23df6e47212").await;
    assert!(rejected, "Timeout waiting for OrderRejected event");
    assert!(suite.log_capture.contains("reason=Asset not supported"));
}

// #[tokio::test]
// async fn test_order_processed() {
//     let suite = get_tests_suite().await;

//     let provider = ProviderBuilder::new()
//         .wallet(suite.evm_signer.clone())
//         .connect_http(suite.anvil.endpoint_url());

//     let contract = OrderBook::new(suite.contract_address, &provider);

//     let builder = contract.openOrder(OrderParams {
//         tokenIn: suite.tokens[0].into(),
//         destChainId: 11155111,
//         tokenOut: FixedBytes::from(decode_evm_address(suite.tokens[1])),
//         amountIn: 1000000,
//         amountOut: 1000000,
//         recipient: FixedBytes::from(decode_evm_address(suite.evm_signer.address())),
//         fillDeadline: u32::MAX,
//         solver: FixedBytes::from([0u8; 32]),
//     });

//     builder
//         .send()
//         .await
//         .expect("Failed to send openOrder transaction")
//         .get_receipt()
//         .await
//         .expect("Failed to confirm transaction");

//     // Wait for the solver to process the order
//     assert!(
//         wait_for_log(&suite.log_capture, "event=\"OrderCreated\" order_id=ec224b6df9436835d1e2c68a8b0f36d6e8e40ad4da250a1102eb90d9822ea520").await,
//         "Timeout waiting for OrderCreated event"
//     );
//     assert!(
//         wait_for_log(&suite.log_capture, "Building fillOrder transaction").await,
//         "Timeout waiting for fillOrder transaction log"
//     );
// }

async fn wait_for_log(capture: &tracing_capture::CaptureLayer, substring: &str) -> bool {
    let timeout = Duration::from_secs(5);
    let poll_interval = Duration::from_millis(10);
    let start = tokio::time::Instant::now();

    while start.elapsed() < timeout {
        if capture.contains(substring) {
            return true;
        }
        sleep(poll_interval).await;
    }

    false
}

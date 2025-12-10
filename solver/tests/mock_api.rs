use alloy::primitives::Address;
use solver::utils::chain_from_id;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct Asset {
    pub address: Address,
    pub chain_id: u32,
    pub symbol: String,
}

// Wrapper to make ServerGuard Send
pub struct SendServerGuard(Arc<Mutex<mockito::ServerGuard>>);

unsafe impl Send for SendServerGuard {}
unsafe impl Sync for SendServerGuard {}

impl Asset {
    fn to_json(&self) -> String {
        format!(
            r#"{{
                "chain": "{}",
                "address": "{}",
                "symbol": "{}",
                "icon": "",
                "name": "{}",
                "decimals": 6,
                "m0Extension": false,
                "runtime": "evm"
            }}"#,
            chain_from_id(self.chain_id),
            self.address,
            self.symbol,
            self.symbol
        )
    }
}

pub async fn mock_api_with_assets(assets: Vec<Asset>) -> (SendServerGuard, String) {
    let mut server = mockito::Server::new_async().await;
    let assets_response: Vec<String> = assets.into_iter().map(|a| a.to_json()).collect();
    let body = format!("[{}]", assets_response.join(","));

    // Assets endpoint
    let _ = server
        .mock("GET", "/supported-assets")
        .with_status(200)
        .with_body(body)
        .create();

    let url = server.url();
    (SendServerGuard(Arc::new(Mutex::new(server))), url)
}

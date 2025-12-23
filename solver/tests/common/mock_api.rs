use alloy::primitives::Address;
use mockito::ServerGuard;
use solver::utils::chain_from_id;

#[derive(Clone, Debug)]
pub struct Asset {
    pub address: Address,
    pub chain_id: u32,
    pub symbol: String,
}

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
                "m0Extension": true,
                "runtime": "evm"
            }}"#,
            chain_from_id(self.chain_id),
            self.address,
            self.symbol,
            self.symbol
        )
    }
}

pub async fn mock_api_with_assets(assets: Vec<Asset>) -> ServerGuard {
    let mut server = mockito::Server::new_async().await;

    let assets_response: Vec<String> = if assets.len() > 0 {
        assets.into_iter().map(|a| a.to_json()).collect()
    } else {
        [1, 8453]
            .iter()
            .map(|&chain_id| Asset {
                address: "0x437cc33344a0B27A429f795ff6B469C72698B291"
                    .parse()
                    .unwrap(),
                chain_id,
                symbol: "wM".to_string(),
            })
            .map(|a| a.to_json())
            .collect()
    };

    let body = format!("[{}]", assets_response.join(","));

    // Assets endpoint
    let _ = server
        .mock("GET", "/supported-assets")
        .with_status(200)
        .with_body(body)
        .create();

    server
}

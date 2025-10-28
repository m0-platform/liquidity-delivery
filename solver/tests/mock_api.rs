pub struct AssetConfig {
    pub address: String,
    pub chain: String,
    pub symbol: String,
}

impl AssetConfig {
    pub fn new(
        address: impl Into<String>,
        chain: impl Into<String>,
        symbol: impl Into<String>,
    ) -> Self {
        Self {
            address: address.into(),
            chain: chain.into(),
            symbol: symbol.into(),
        }
    }

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
            self.chain, self.address, self.symbol, self.symbol
        )
    }
}

pub async fn mock_api_with_assets(additional_assets: Vec<AssetConfig>) -> mockito::ServerGuard {
    let mut server = mockito::Server::new_async().await;

    let mut assets = vec![
        r#"{
            "chain": "Ethereum",
            "address": "0x437cc33344a0B27A429f795ff6B469C72698B291",
            "symbol": "wM",
            "icon": "",
            "name": "Wrapped $M",
            "decimals": 6,
            "m0Extension": true,
            "runtime": "evm"
        }"#
        .to_string(),
        r#"{
            "chain": "Solana",
            "address": "mzeroXDoBpRVhnEXBra27qzAMdxgpWVY3DzQW7xMVJp",
            "symbol": "wM", 
            "icon": "",
            "name": "Wrapped $M",
            "decimals": 6,
            "m0Extension": true,
            "runtime": "svm"
        }"#
        .to_string(),
    ];

    assets.extend(additional_assets.into_iter().map(|a| a.to_json()));

    let body = format!("[{}]", assets.join(","));

    // Assets endpoint
    let _ = server
        .mock("GET", "/supported-assets")
        .with_status(200)
        .with_body(body)
        .create();

    server
}

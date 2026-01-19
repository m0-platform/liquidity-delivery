export type NetworkType = "local" | "devnet" | "mainnet";

export interface NetworkConfig {
  quoterUrl: string;
  assetsApiUrl: string;
  ordersApiUrl: string;
  ethereumRpc: string;
  solanaRpc: string;
  baseRpc?: string;
}

const configs: Record<NetworkType, NetworkConfig> = {
  local: {
    quoterUrl: import.meta.env.VITE_QUOTER_URL_LOCAL,
    assetsApiUrl: import.meta.env.VITE_LIQUIDITY_API_LOCAL,
    ordersApiUrl: import.meta.env.VITE_ORDERS_API_LOCAL,
    ethereumRpc: import.meta.env.VITE_LOCALNET_ETHEREUM_RPC,
    solanaRpc: import.meta.env.VITE_LOCALNET_SOLANA_RPC,
    baseRpc: import.meta.env.VITE_LOCALNET_BASE_RPC,
  },
  devnet: {
    quoterUrl: import.meta.env.VITE_QUOTER_URL_DEVNET,
    assetsApiUrl: import.meta.env.VITE_LIQUIDITY_API_DEVNET,
    ordersApiUrl: import.meta.env.VITE_ORDERS_API_DEVNET,
    ethereumRpc: import.meta.env.VITE_DEVNET_ETHEREUM_RPC,
    solanaRpc: import.meta.env.VITE_DEVNET_SOLANA_RPC,
    baseRpc: import.meta.env.VITE_DEVNET_BASE_RPC,
  },
  mainnet: {
    quoterUrl: import.meta.env.VITE_QUOTER_URL_MAINNET,
    assetsApiUrl: import.meta.env.VITE_LIQUIDITY_API_MAINNET,
    ethereumRpc: import.meta.env.VITE_MAINNET_ETHEREUM_RPC,
    ordersApiUrl: import.meta.env.VITE_ORDERS_API_LOCAL,
    solanaRpc: import.meta.env.VITE_MAINNET_SOLANA_RPC,
    baseRpc: import.meta.env.VITE_MAINNET_BASE_RPC,
  },
};

export function getNetworkConfig(network: NetworkType): NetworkConfig {
  return configs[network];
}

export function getQuoterUrl(network: NetworkType): string {
  return configs[network].quoterUrl;
}

export function getAssetsApiUrl(network: NetworkType): string {
  return configs[network].assetsApiUrl;
}

export function getOrdersUrl(network: NetworkType): string {
  return configs[network].ordersApiUrl;
}

export function getEthereumRpc(network: NetworkType): string {
  return configs[network].ethereumRpc;
}

export function getSolanaRpc(network: NetworkType): string {
  return configs[network].solanaRpc;
}

export function getBaseRpc(network: NetworkType): string | undefined {
  return configs[network].baseRpc;
}

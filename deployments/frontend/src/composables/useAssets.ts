import { ref, computed, toValue, watch, type MaybeRef } from "vue";
import { getAssetsApiUrl, type NetworkType } from "../config/network";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

export interface Asset {
  ticker: string;
  symbol: string;
  name: string;
  icon: string;
  address: string;
  decimals: number;
  chainId: number;
  chain: string;
  runtime: "evm" | "svm";
  extensionTokenProgramId?: string;
}

// Response type from the API
interface AssetResponse {
  chain: string;
  chainId?: number;
  address: string;
  symbol: string;
  icon: string;
  name: string;
  decimals: number;
  m0Extension: boolean;
  runtime: "evm" | "svm";
  extensionTokenProgramId?: string;
}

// Map chain names from API to numeric chain IDs
// The API returns chain names (e.g., "Sepolia", "Solana") but not numeric IDs
const chainNameToId: Record<NetworkType, Record<string, number>> = {
  local: {
    Ethereum: 1,
    Base: 8453,
    Solana: 1399811149,
  },
  devnet: {
    Sepolia: 11155111,
    BaseSepolia: 84532,
    "Base Sepolia": 84532,
    ArbitrumSepolia: 421614,
    "Arbitrum Sepolia": 421614,
    Solana: 1399811150,
    SolanaDevnet: 1399811150,
    "Solana Devnet": 1399811150,
  },
  mainnet: {
    Ethereum: 1,
    Base: 8453,
    Arbitrum: 42161,
    Solana: 1399811149,
  },
};

function getChainIdFromName(chainName: string, network: NetworkType): number {
  const chainId = chainNameToId[network]?.[chainName];
  if (chainId === undefined) {
    console.warn(`Unknown chain name "${chainName}" for network "${network}"`);
    return 0;
  }
  return chainId;
}

export function useAssets(networkRef: MaybeRef<NetworkType>) {
  const assets = ref<Asset[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const assetsApiUrl = computed(() => getAssetsApiUrl(toValue(networkRef)));

  async function fetchAssets(): Promise<Asset[]> {
    loading.value = true;
    error.value = null;

    try {
      const response = await fetch(`${assetsApiUrl.value}/supported-assets`);

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const data: AssetResponse[] = await response.json();

      const network = toValue(networkRef);

      // Map API response to frontend Asset interface
      assets.value = data.map((item) => ({
        ticker: item.symbol, // Use symbol as ticker
        symbol: item.symbol,
        name: item.name,
        icon: item.icon,
        address: item.address,
        decimals: item.decimals,
        chainId: item.chainId ?? getChainIdFromName(item.chain, network),
        chain: item.chain,
        runtime: item.runtime,
        extensionTokenProgramId:
          item.extensionTokenProgramId ?? TOKEN_PROGRAM_ID.toBase58(),
      }));

      return assets.value;
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to fetch assets";
      return [];
    } finally {
      loading.value = false;
    }
  }

  function getAssetsForChain(chainId: number): Asset[] {
    return assets.value.filter((asset) => asset.chainId === chainId);
  }

  function getAssetForChain(
    ticker: string,
    chainId: number,
  ): Asset | undefined {
    return assets.value.find(
      (asset) => asset.ticker === ticker && asset.chainId === chainId,
    );
  }

  function getUniqueTickers(): string[] {
    const tickers = new Set(assets.value.map((asset) => asset.ticker));
    return Array.from(tickers);
  }

  function getTickersForChain(chainId: number): string[] {
    const tickers = new Set(
      assets.value
        .filter((asset) => asset.chainId === chainId)
        .map((asset) => asset.ticker)
    );
    return Array.from(tickers);
  }

  // Fetch assets when network changes (and on initial mount via immediate: true)
  watch(
    () => toValue(networkRef),
    () => {
      // Clear stale assets from previous network to prevent chainId mismatches
      assets.value = [];
      fetchAssets();
    },
    { immediate: true },
  );

  return {
    assets,
    loading,
    error,
    fetchAssets,
    getAssetsForChain,
    getAssetForChain,
    getUniqueTickers,
    getTickersForChain,
  };
}

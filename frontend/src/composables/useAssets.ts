import { ref, onMounted } from 'vue'

export interface Asset {
  ticker: string
  symbol: string
  name: string
  icon: string
  address: string
  decimals: number
  chainId: number
  chain: string
  runtime: 'evm' | 'svm'
}

// Response type from the mock API
interface MockAssetResponse {
  chain: string
  chainId: number
  address: string
  symbol: string
  icon: string
  name: string
  decimals: number
  m0Extension: boolean
  runtime: 'evm' | 'svm'
}

export function useAssets() {
  const assets = ref<Asset[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const mockApiUrl = import.meta.env.VITE_MOCK_API_URL || 'http://localhost:8080'

  async function fetchAssets(): Promise<Asset[]> {
    loading.value = true
    error.value = null

    try {
      const response = await fetch(`${mockApiUrl}/supported-assets`)

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }

      const data: MockAssetResponse[] = await response.json()

      // Map mock API response to frontend Asset interface
      // No flattening needed - mock API already returns flattened data
      assets.value = data.map((item) => ({
        ticker: item.symbol, // Use symbol as ticker
        symbol: item.symbol,
        name: item.name,
        icon: item.icon,
        address: item.address,
        decimals: item.decimals,
        chainId: item.chainId,
        chain: item.chain, // Already provided by mock API
        runtime: item.runtime, // Already provided by mock API
      }))

      return assets.value
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to fetch assets'
      return []
    } finally {
      loading.value = false
    }
  }

  function getAssetsForChain(chainId: number): Asset[] {
    return assets.value.filter((asset) => asset.chainId === chainId)
  }

  function getAssetForChain(ticker: string, chainId: number): Asset | undefined {
    return assets.value.find((asset) => asset.ticker === ticker && asset.chainId === chainId)
  }

  function getUniqueTickers(): string[] {
    const tickers = new Set(assets.value.map((asset) => asset.ticker))
    return Array.from(tickers)
  }

  onMounted(() => {
    fetchAssets()
  })

  return {
    assets,
    loading,
    error,
    fetchAssets,
    getAssetsForChain,
    getAssetForChain,
    getUniqueTickers,
  }
}

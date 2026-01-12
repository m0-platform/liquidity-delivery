import { ref, onMounted } from 'vue'

export interface Asset {
  ticker: string
  name: string
  icon: string
  address: string
  chain_ids: number[]
}

export function useAssets() {
  const assets = ref<Asset[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const quoterUrl = import.meta.env.VITE_QUOTER_URL || 'http://localhost:3000'

  async function fetchAssets(): Promise<Asset[]> {
    loading.value = true
    error.value = null

    try {
      const response = await fetch(`${quoterUrl}/assets`)

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }

      const data = await response.json()
      assets.value = data
      return data
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to fetch assets'
      return []
    } finally {
      loading.value = false
    }
  }

  function getAssetsForChain(chainId: number): Asset[] {
    return assets.value.filter(asset => asset.chain_ids.includes(chainId))
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
  }
}

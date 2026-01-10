import { ref } from 'vue'

interface QuoteRequest {
  srcChainId: number
  dstChainId: number
  srcToken: string
  dstToken: string
  amount: string
}

interface QuoteResponse {
  amountOut: string
  rate: string
  fee: string
  estimatedTime: string
  solver?: string
}

export function useQuoter() {
  const loading = ref(false)
  const error = ref<string | null>(null)
  const quote = ref<QuoteResponse | null>(null)

  const quoterUrl = import.meta.env.VITE_QUOTER_URL || 'http://localhost:3000'

  async function getQuote(request: QuoteRequest): Promise<QuoteResponse | null> {
    loading.value = true
    error.value = null
    quote.value = null

    try {
      const response = await fetch(`${quoterUrl}/quote`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          src_chain_id: request.srcChainId,
          dst_chain_id: request.dstChainId,
          src_token: request.srcToken,
          dst_token: request.dstToken,
          amount: request.amount,
        }),
      })

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}))
        throw new Error(errorData.message || `HTTP ${response.status}`)
      }

      const data = await response.json()

      quote.value = {
        amountOut: data.amount_out || data.amountOut || '0',
        rate: data.rate || '1.00',
        fee: data.fee || '0',
        estimatedTime: data.estimated_time || data.estimatedTime || '~30s',
        solver: data.solver,
      }

      return quote.value

    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to get quote'
      return null
    } finally {
      loading.value = false
    }
  }

  return {
    loading,
    error,
    quote,
    getQuote,
  }
}

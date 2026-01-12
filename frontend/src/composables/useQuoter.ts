import { ref } from "vue";

interface QuoteRequest {
  srcChainId: number;
  dstChainId: number;
  srcToken: string;
  dstToken: string;
  amount: string;
}

interface QuoteResponse {
  amountOut: string;
  rate: number;
  fee: string;
  estimatedTime: string;
  solver?: string;
}

export function useQuoter() {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const quote = ref<QuoteResponse | null>(null);

  const quoterUrl = import.meta.env.VITE_QUOTER_URL || "http://localhost:3000";

  async function getQuote(
    request: QuoteRequest
  ): Promise<QuoteResponse | null> {
    loading.value = true;
    error.value = null;
    quote.value = null;

    try {
      const response = await fetch(`${quoterUrl}/quote`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          input_chain_id: request.srcChainId,
          output_chain_id: request.dstChainId,
          input_token: request.srcToken,
          output_token: request.dstToken,
          amount_in: request.amount,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(errorData.message || `HTTP ${response.status}`);
      }

      const data = await response.json();

      quote.value = {
        amountOut: data.amount_out || "0",
        rate: parseInt(request.amount) / parseInt(data.amount_out) || 1,
        fee: data.fee_bps || "0",
        estimatedTime: data.estimated_time || data.estimatedTime || "~30s",
        solver: data.solver,
      };

      return quote.value;
    } catch (err) {
      error.value = err instanceof Error ? err.message : "Failed to get quote";
      return null;
    } finally {
      loading.value = false;
    }
  }

  return {
    loading,
    error,
    quote,
    getQuote,
  };
}

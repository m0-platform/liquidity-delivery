import { ref } from "vue";

interface QuoteRequest {
  srcChainId: number;
  dstChainId: number;
  srcToken: string;
  dstToken: string;
  amount: string;
  senderAddress?: string;
  recipient?: string;
}

export interface EvmTransaction {
  to: string;
  data: string;
  value: string;
}

export interface QuoteResponse {
  amountOut: string;
  rate: number;
  fee: string;
  estimatedTime: string;
  solver?: string;
  orderId?: string;
  evmTransaction?: EvmTransaction;
  approvalTransaction?: EvmTransaction;
  svmTransaction?: string;
  nonce?: number;
  orderbookAddress?: string;
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
          amount_in: parseInt(request.amount),
          sender_address: request.senderAddress,
          recipient: request.recipient,
        }),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(errorData.message || `HTTP ${response.status}`);
      }

      const data = await response.json();

      // Response is an array of quotes, get the first one
      const quoteData = Array.isArray(data) ? data[0] : data;

      if (!quoteData) {
        throw new Error("No quotes available");
      }

      // Check if quote was rejected
      if (quoteData.rejected) {
        throw new Error(quoteData.reject_reason || "Quote rejected");
      }

      const outputAmount = String(
        quoteData.output_amount ?? quoteData.amount_out ?? "0"
      );
      const inputAmount = parseInt(request.amount) || 1;
      const outputAmountNum = parseInt(outputAmount) || 1;

      quote.value = {
        amountOut: outputAmount,
        rate: outputAmountNum / inputAmount,
        fee: String(quoteData.fee_bps ?? "0"),
        estimatedTime: quoteData.est_fill_time_seconds
          ? `~${quoteData.est_fill_time_seconds}s`
          : "~30s",
        solver: quoteData.solver_address || quoteData.solver,
        orderId: quoteData.order_id,
        evmTransaction: quoteData.evm_transaction,
        approvalTransaction: quoteData.approval_transaction,
        svmTransaction: quoteData.svm_transaction,
        nonce: quoteData.nonce,
        orderbookAddress: quoteData.orderbook_address,
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

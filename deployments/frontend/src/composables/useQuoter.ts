import { ref, computed, toValue, type MaybeRef } from "vue";
import { getQuoterUrl, type NetworkType } from "../config/network";

interface QuoteRequest {
  srcChain: string;
  dstChain: string;
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
  chainId?: number;
}

export interface QuoteResponse {
  amountOut: string;
  rate: number;
  fee: string;
  estimatedTime: string;
  solver?: string;
  evmTransaction?: EvmTransaction;
  approvalTransaction?: EvmTransaction;
  svmTransaction?: string;
  orderId?: string;
}

export function useQuoter(networkRef: MaybeRef<NetworkType>) {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const quote = ref<QuoteResponse | null>(null);

  const quoterUrl = computed(() => getQuoterUrl(toValue(networkRef)));

  async function getQuote(
    request: QuoteRequest
  ): Promise<QuoteResponse | null> {
    loading.value = true;
    error.value = null;
    quote.value = null;

    try {
      const response = await fetch(`${quoterUrl.value}/quote`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          route: {
            source: {
              chain: request.srcChain,
              address: request.srcToken,
            },
            destination: {
              chain: request.dstChain,
              address: request.dstToken,
            },
          },
          amountIn: request.amount,
          sender: request.senderAddress || "",
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

      const outputAmount = String(quoteData.amountOut ?? "0");
      const inputAmount = parseInt(request.amount) || 1;
      const outputAmountNum = parseInt(outputAmount) || 1;

      // Extract transactions from payloads
      interface PayloadData {
        type: string;
        chain?: string;
        chainId?: number;
        to: string;
        data: string;
        value: string;
        transaction?: string;
      }
      interface Payload {
        provider: string;
        annotation: string;
        data: PayloadData;
      }

      const payloads: Payload[] = quoteData.payloads || [];

      // Filter EVM payloads and separate approval from main transaction
      const evmPayloads = payloads.filter((p) => p.data?.type === "evm");

      // Approval transaction has "Approve" in annotation
      const approvalPayload = evmPayloads.find((p) =>
        p.annotation?.toLowerCase().includes("approve")
      );

      // Main transaction is the non-approval EVM payload
      const mainEvmPayload = evmPayloads.find(
        (p) => !p.annotation?.toLowerCase().includes("approve")
      );

      // SVM transaction
      const svmPayload = payloads.find((p) => p.data?.type === "svm");

      quote.value = {
        amountOut: outputAmount,
        rate: outputAmountNum / inputAmount,
        fee: "0",
        estimatedTime: quoteData.estFillTime
          ? `~${quoteData.estFillTime}s`
          : "~30s",
        solver: payloads[0]?.provider,
        approvalTransaction: approvalPayload?.data
          ? {
              to: approvalPayload.data.to,
              data: approvalPayload.data.data,
              value: approvalPayload.data.value,
              chainId: approvalPayload.data.chainId,
            }
          : undefined,
        evmTransaction: mainEvmPayload?.data
          ? {
              to: mainEvmPayload.data.to,
              data: mainEvmPayload.data.data,
              value: mainEvmPayload.data.value,
              chainId: mainEvmPayload.data.chainId,
            }
          : undefined,
        svmTransaction: svmPayload?.data?.transaction,
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

import { ref } from "vue";
import {
  sendTransaction,
  getAccount,
  waitForTransactionReceipt,
} from "@wagmi/core";
import { Connection, Transaction, VersionedTransaction } from "@solana/web3.js";
import { wagmiConfig } from "../wallets";
import { keccak256, toHex } from "viem";
import type { Wallet } from "ethers";
import type { Keypair } from "@solana/web3.js";
import type { EvmTransaction } from "./useQuoter";
import type Solflare from "@solflare-wallet/sdk";

// OrderOpened event signature: OrderOpened(bytes32 indexed orderId, address sender, address tokenIn, uint128 amountIn, uint32 indexed destChainId, bytes32 tokenOut, uint128 amountOut, bytes32 indexed solver)
const ORDER_OPENED_EVENT_SIGNATURE = keccak256(
  toHex(
    "OrderOpened(bytes32,address,address,uint128,uint32,bytes32,uint128,bytes32)",
  ),
);

export interface SwapResult {
  orderId: string;
  txHash: string;
  approvalTxHash?: string;
}

export type ChainType = "evm" | "svm";

export function useSwap() {
  const loading = ref(false);
  const error = ref<string | null>(null);

  /**
   * Send a single EVM transaction and optionally wait for confirmation
   */
  async function sendEvmTransaction(
    tx: EvmTransaction,
    localSigner?: Wallet | null,
    waitForConfirmation = false,
  ): Promise<string> {
    if (localSigner) {
      // Local mode - sign and send directly with ethers
      const result = await localSigner.sendTransaction({
        to: tx.to,
        data: tx.data,
        value: tx.value,
      });
      if (waitForConfirmation) {
        await result.wait();
      }
      return result.hash;
    } else {
      // External wallet mode - use wagmi
      const account = getAccount(wagmiConfig);
      if (!account.address) {
        throw new Error("EVM wallet not connected");
      }

      const txHash = await sendTransaction(wagmiConfig, {
        to: tx.to as `0x${string}`,
        data: tx.data as `0x${string}`,
        value: BigInt(tx.value),
      });

      if (waitForConfirmation) {
        await waitForTransactionReceipt(wagmiConfig, { hash: txHash });
      }

      return txHash;
    }
  }

  /**
   * Parse the actual order ID from EVM transaction receipt logs
   */
  function parseOrderIdFromLogs(
    logs: ReadonlyArray<{ topics: readonly string[] }>,
  ): string | null {
    for (const log of logs) {
      // Check if this is an OrderOpened event (first topic is the event signature)
      if (
        log.topics[0]?.toLowerCase() ===
        ORDER_OPENED_EVENT_SIGNATURE.toLowerCase()
      ) {
        // The orderId is the second topic (first indexed parameter)
        const orderId = log.topics[1];
        if (orderId) {
          return orderId;
        }
      }
    }
    return null;
  }

  /**
   * Execute an EVM swap by sending approval (if needed) and the main transaction
   */
  async function executeEvmSwap(
    evmTransaction: EvmTransaction,
    orderId: string,
    approvalTransaction?: EvmTransaction,
    localSigner?: Wallet | null,
  ): Promise<SwapResult> {
    loading.value = true;
    error.value = null;

    try {
      let approvalTxHash: string | undefined;

      // Send approval transaction first if needed and wait for it to be mined
      if (approvalTransaction) {
        approvalTxHash = await sendEvmTransaction(
          approvalTransaction,
          localSigner,
          true,
        );
      }

      // Send the main open order transaction
      const txHash = await sendEvmTransaction(evmTransaction, localSigner);

      // Wait for the transaction receipt to get the actual order ID from logs
      let actualOrderId = orderId;
      if (localSigner) {
        // For local signer, wait for the transaction to be mined and get receipt
        const txResponse = await localSigner.provider?.getTransaction(txHash);
        const receipt = await txResponse?.wait();
        if (receipt?.logs) {
          const parsedOrderId = parseOrderIdFromLogs(receipt.logs);
          if (parsedOrderId) {
            actualOrderId = parsedOrderId;
          }
        }
      } else {
        // For wagmi, wait for receipt
        const receipt = await waitForTransactionReceipt(wagmiConfig, {
          hash: txHash as `0x${string}`,
        });
        if (receipt?.logs) {
          const parsedOrderId = parseOrderIdFromLogs(receipt.logs);
          if (parsedOrderId) {
            actualOrderId = parsedOrderId;
          }
        }
      }

      return { orderId: actualOrderId, txHash, approvalTxHash };
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to execute EVM swap";
      throw err;
    } finally {
      loading.value = false;
    }
  }

  /**
   * Execute an SVM swap by deserializing, signing, and sending the transaction
   */
  async function executeSvmSwap(
    transactionBase64: string,
    orderId: string,
    rpcUrl: string,
    localKeypair?: Keypair | null,
    solflareWallet?: Solflare | null,
  ): Promise<SwapResult> {
    loading.value = true;
    error.value = null;

    try {
      const connection = new Connection(rpcUrl);

      // Decode the base64 transaction
      const transactionBuffer = Buffer.from(transactionBase64, "base64");

      let txHash: string;

      if (localKeypair) {
        // Local mode - sign with keypair
        // Try to deserialize as legacy transaction first
        let transaction: Transaction | VersionedTransaction;
        try {
          transaction = Transaction.from(transactionBuffer);
          transaction.sign(localKeypair);
        } catch {
          // Try versioned transaction
          transaction = VersionedTransaction.deserialize(transactionBuffer);
          transaction.sign([localKeypair]);
        }

        const signature = await connection.sendRawTransaction(
          transaction.serialize(),
        );
        txHash = signature;
      } else {
        // External wallet mode - use Solflare
        if (!solflareWallet || !solflareWallet.isConnected) {
          throw new Error("Solflare wallet not connected");
        }

        // Deserialize and sign with Solflare
        let transaction: Transaction | VersionedTransaction;
        try {
          transaction = Transaction.from(transactionBuffer);
        } catch {
          transaction = VersionedTransaction.deserialize(transactionBuffer);
        }

        const signedTransaction =
          await solflareWallet.signTransaction(transaction);
        const signature = await connection.sendRawTransaction(
          signedTransaction.serialize(),
        );
        txHash = signature;
      }

      // Confirm the transaction and get the actual order ID from logs
      try {
        await connection.confirmTransaction(txHash, "confirmed");
      } catch (confirmErr) {
        console.warn(
          "Failed to confirm transaction or parse logs, using predicted order ID:",
          confirmErr,
        );
      }

      return { orderId, txHash };
    } catch (err) {
      error.value =
        err instanceof Error ? err.message : "Failed to execute SVM swap";
      throw err;
    } finally {
      loading.value = false;
    }
  }

  /**
   * Execute a swap based on chain type
   */
  async function executeSwap(
    chainType: ChainType,
    options: {
      evmTransaction?: EvmTransaction;
      approvalTransaction?: EvmTransaction;
      svmTransaction?: string;
      orderId: string;
      svmRpcUrl?: string;
      localEvmSigner?: Wallet | null;
      localSvmKeypair?: Keypair | null;
      solflareWallet?: Solflare | null;
    },
  ): Promise<SwapResult> {
    const {
      evmTransaction,
      approvalTransaction,
      svmTransaction,
      orderId,
      svmRpcUrl,
      localEvmSigner,
      localSvmKeypair,
      solflareWallet,
    } = options;

    if (chainType === "evm") {
      if (!evmTransaction) {
        throw new Error("No EVM transaction data available");
      }
      return executeEvmSwap(
        evmTransaction,
        orderId,
        approvalTransaction,
        localEvmSigner,
      );
    } else {
      if (!svmTransaction) {
        throw new Error("No SVM transaction data available");
      }
      if (!svmRpcUrl) {
        throw new Error("No SVM RPC URL provided");
      }
      return executeSvmSwap(
        svmTransaction,
        orderId,
        svmRpcUrl,
        localSvmKeypair,
        solflareWallet,
      );
    }
  }

  return {
    loading,
    error,
    executeSwap,
    executeEvmSwap,
    executeSvmSwap,
  };
}

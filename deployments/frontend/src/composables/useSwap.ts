import { ref } from 'vue'
import { sendTransaction, getAccount, waitForTransactionReceipt } from '@wagmi/core'
import { Connection, Transaction, VersionedTransaction } from '@solana/web3.js'
import { wagmiConfig } from '../wallets'
import type { Wallet } from 'ethers'
import type { Keypair } from '@solana/web3.js'
import type { EvmTransaction } from './useQuoter'
import type Solflare from '@solflare-wallet/sdk'

export interface SwapResult {
  orderId: string
  txHash: string
  approvalTxHash?: string
}

export type ChainType = 'evm' | 'svm'

export function useSwap() {
  const loading = ref(false)
  const error = ref<string | null>(null)

  /**
   * Send a single EVM transaction and optionally wait for confirmation
   */
  async function sendEvmTransaction(
    tx: EvmTransaction,
    localSigner?: Wallet | null,
    waitForConfirmation = false
  ): Promise<string> {
    if (localSigner) {
      // Local mode - sign and send directly with ethers
      const result = await localSigner.sendTransaction({
        to: tx.to,
        data: tx.data,
        value: tx.value,
      })
      if (waitForConfirmation) {
        await result.wait()
      }
      return result.hash
    } else {
      // External wallet mode - use wagmi
      const account = getAccount(wagmiConfig)
      if (!account.address) {
        throw new Error('EVM wallet not connected')
      }

      const txHash = await sendTransaction(wagmiConfig, {
        to: tx.to as `0x${string}`,
        data: tx.data as `0x${string}`,
        value: BigInt(tx.value),
      })

      if (waitForConfirmation) {
        await waitForTransactionReceipt(wagmiConfig, { hash: txHash })
      }

      return txHash
    }
  }

  /**
   * Execute an EVM swap by sending approval (if needed) and the main transaction
   */
  async function executeEvmSwap(
    evmTransaction: EvmTransaction,
    orderId: string,
    approvalTransaction?: EvmTransaction,
    localSigner?: Wallet | null
  ): Promise<SwapResult> {
    loading.value = true
    error.value = null

    try {
      let approvalTxHash: string | undefined

      // Send approval transaction first if needed and wait for it to be mined
      if (approvalTransaction) {
        approvalTxHash = await sendEvmTransaction(approvalTransaction, localSigner, true)
      }

      // Send the main open order transaction
      const txHash = await sendEvmTransaction(evmTransaction, localSigner)

      return { orderId, txHash, approvalTxHash }
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to execute EVM swap'
      throw err
    } finally {
      loading.value = false
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
    solflareWallet?: Solflare | null
  ): Promise<SwapResult> {
    loading.value = true
    error.value = null

    try {
      const connection = new Connection(rpcUrl)

      // Decode the base64 transaction
      const transactionBuffer = Buffer.from(transactionBase64, 'base64')

      let txHash: string

      if (localKeypair) {
        // Local mode - sign with keypair
        // Try to deserialize as legacy transaction first
        let transaction: Transaction | VersionedTransaction
        try {
          transaction = Transaction.from(transactionBuffer)
          transaction.sign(localKeypair)
        } catch {
          // Try versioned transaction
          transaction = VersionedTransaction.deserialize(transactionBuffer)
          transaction.sign([localKeypair])
        }

        const signature = await connection.sendRawTransaction(
          transaction.serialize()
        )
        txHash = signature
      } else {
        // External wallet mode - use Solflare
        if (!solflareWallet || !solflareWallet.isConnected) {
          throw new Error('Solflare wallet not connected')
        }

        // Deserialize and sign with Solflare
        let transaction: Transaction | VersionedTransaction
        try {
          transaction = Transaction.from(transactionBuffer)
        } catch {
          transaction = VersionedTransaction.deserialize(transactionBuffer)
        }

        const signedTransaction = await solflareWallet.signTransaction(transaction)
        const signature = await connection.sendRawTransaction(
          signedTransaction.serialize()
        )
        txHash = signature
      }

      return { orderId, txHash }
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to execute SVM swap'
      throw err
    } finally {
      loading.value = false
    }
  }

  /**
   * Execute a swap based on chain type
   */
  async function executeSwap(
    chainType: ChainType,
    options: {
      evmTransaction?: EvmTransaction
      approvalTransaction?: EvmTransaction
      svmTransaction?: string
      orderId: string
      svmRpcUrl?: string
      localEvmSigner?: Wallet | null
      localSvmKeypair?: Keypair | null
      solflareWallet?: Solflare | null
    }
  ): Promise<SwapResult> {
    const {
      evmTransaction,
      approvalTransaction,
      svmTransaction,
      orderId,
      svmRpcUrl,
      localEvmSigner,
      localSvmKeypair,
      solflareWallet
    } = options

    if (chainType === 'evm') {
      if (!evmTransaction) {
        throw new Error('No EVM transaction data available')
      }
      return executeEvmSwap(evmTransaction, orderId, approvalTransaction, localEvmSigner)
    } else {
      if (!svmTransaction) {
        throw new Error('No SVM transaction data available')
      }
      if (!svmRpcUrl) {
        throw new Error('No SVM RPC URL provided')
      }
      return executeSvmSwap(svmTransaction, orderId, svmRpcUrl, localSvmKeypair, solflareWallet)
    }
  }

  return {
    loading,
    error,
    executeSwap,
    executeEvmSwap,
    executeSvmSwap,
  }
}

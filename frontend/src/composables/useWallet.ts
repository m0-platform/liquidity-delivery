import { ref, computed, watchEffect } from 'vue'
import { BrowserProvider } from 'ethers'
import { Connection, PublicKey } from '@solana/web3.js'

declare global {
  interface Window {
    ethereum?: {
      isMetaMask?: boolean
      request: (args: { method: string; params?: unknown[] }) => Promise<unknown>
      on: (event: string, handler: (...args: unknown[]) => void) => void
      removeListener: (event: string, handler: (...args: unknown[]) => void) => void
    }
    solana?: {
      isPhantom?: boolean
      connect: () => Promise<{ publicKey: PublicKey }>
      disconnect: () => Promise<void>
      on: (event: string, handler: (...args: unknown[]) => void) => void
      publicKey?: PublicKey
    }
  }
}

export function useWallet(network: 'local' | 'devnet' | 'mainnet') {
  const evmAddress = ref<string | null>(null)
  const svmAddress = ref<string | null>(null)
  const error = ref<string | null>(null)

  const evmConnected = computed(() => !!evmAddress.value)
  const svmConnected = computed(() => !!svmAddress.value)

  // EVM chain configurations
  const evmChainConfig = computed(() => {
    switch (network) {
      case 'local':
        return {
          chainId: '0x7A69', // 31337
          chainName: 'Anvil Local',
          rpcUrls: [import.meta.env.VITE_ANVIL_RPC || 'http://localhost:8545'],
        }
      case 'devnet':
        return {
          chainId: '0xAA36A7', // 11155111 (Sepolia)
          chainName: 'Sepolia',
          rpcUrls: ['https://sepolia.gateway.tenderly.co'],
        }
      case 'mainnet':
        return {
          chainId: '0x1', // 1
          chainName: 'Ethereum',
          rpcUrls: ['https://eth.llamarpc.com'],
        }
    }
  })

  // Solana RPC configurations
  const solanaRpc = computed(() => {
    switch (network) {
      case 'local':
        return import.meta.env.VITE_SURFPOOL_RPC || 'http://localhost:8899'
      case 'devnet':
        return 'https://api.devnet.solana.com'
      case 'mainnet':
        return 'https://api.mainnet-beta.solana.com'
    }
  })

  async function connectEvm(): Promise<void> {
    error.value = null

    if (!window.ethereum?.isMetaMask) {
      error.value = 'MetaMask not detected. Please install MetaMask.'
      return
    }

    try {
      // Request account access
      const accounts = await window.ethereum.request({
        method: 'eth_requestAccounts',
      }) as string[]

      if (accounts.length === 0) {
        error.value = 'No accounts found'
        return
      }

      evmAddress.value = accounts[0]

      // Switch to the correct network
      try {
        await window.ethereum.request({
          method: 'wallet_switchEthereumChain',
          params: [{ chainId: evmChainConfig.value.chainId }],
        })
      } catch (switchError: unknown) {
        // If the chain doesn't exist, add it (for local network)
        if ((switchError as { code: number }).code === 4902 && network === 'local') {
          await window.ethereum.request({
            method: 'wallet_addEthereumChain',
            params: [{
              chainId: evmChainConfig.value.chainId,
              chainName: evmChainConfig.value.chainName,
              rpcUrls: evmChainConfig.value.rpcUrls,
              nativeCurrency: {
                name: 'ETH',
                symbol: 'ETH',
                decimals: 18,
              },
            }],
          })
        }
      }

      // Listen for account changes
      window.ethereum.on('accountsChanged', (accounts: unknown) => {
        const accountList = accounts as string[]
        evmAddress.value = accountList[0] || null
      })

    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to connect MetaMask'
    }
  }

  async function connectSvm(): Promise<void> {
    error.value = null

    if (!window.solana?.isPhantom) {
      error.value = 'Phantom not detected. Please install Phantom wallet.'
      return
    }

    try {
      const response = await window.solana.connect()
      svmAddress.value = response.publicKey.toBase58()

      // Listen for disconnect
      window.solana.on('disconnect', () => {
        svmAddress.value = null
      })

    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to connect Phantom'
    }
  }

  function disconnectEvm(): void {
    evmAddress.value = null
  }

  async function disconnectSvm(): Promise<void> {
    if (window.solana) {
      await window.solana.disconnect()
    }
    svmAddress.value = null
  }

  return {
    evmAddress,
    svmAddress,
    evmConnected,
    svmConnected,
    error,
    connectEvm,
    connectSvm,
    disconnectEvm,
    disconnectSvm,
    evmChainConfig,
    solanaRpc,
  }
}

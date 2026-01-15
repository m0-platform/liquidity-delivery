import { ref, shallowRef, computed, watch, toValue, type MaybeRef, onUnmounted } from 'vue'
import { Wallet, JsonRpcProvider } from 'ethers'
import { Keypair } from '@solana/web3.js'
import { getAccount, watchAccount, disconnect as wagmiDisconnect, connect as wagmiConnect, getConnectors } from '@wagmi/core'
import { wagmiConfig, solflare } from '../wallets'

export type NetworkType = 'local' | 'devnet' | 'mainnet'

export interface WalletState {
  evmAddress: string | null
  svmAddress: string | null
  evmConnected: boolean
  svmConnected: boolean
  isLocal: boolean
  walletType: 'local' | 'external'
  error: string | null
}

export function useWallet(networkRef: MaybeRef<NetworkType>) {
  const evmAddress = ref<string | null>(null)
  const svmAddress = ref<string | null>(null)
  const error = ref<string | null>(null)
  const walletType = ref<'local' | 'external'>('external')

  // Local mode wallet instances (for signing)
  // Use shallowRef to preserve class instances with private properties
  const localEvmWallet = shallowRef<Wallet | null>(null)
  const localSvmKeypair = shallowRef<Keypair | null>(null)

  // Computed that reactively gets the current network
  const currentNetwork = computed(() => toValue(networkRef))
  const isLocal = computed(() => currentNetwork.value === 'local')
  const evmConnected = computed(() => !!evmAddress.value)
  const svmConnected = computed(() => !!svmAddress.value)

  // Track watchers for cleanup
  let unwatchWagmi: (() => void) | null = null
  let solflareListenersSetup = false

  // Setup wagmi watcher for EVM account changes
  function setupWagmiWatcher() {
    if (isLocal.value) return

    // Clean up previous watcher if exists
    unwatchWagmi?.()

    unwatchWagmi = watchAccount(wagmiConfig, {
      onChange(data) {
        if (isLocal.value) return
        walletType.value = 'external'
        evmAddress.value = data.address ?? null
      },
    })

    // Get initial state
    const account = getAccount(wagmiConfig)
    if (account.address) {
      evmAddress.value = account.address
      walletType.value = 'external'
    }
  }

  // Setup Solflare event listeners for Solana account changes
  function setupSolflareListeners() {
    if (isLocal.value || solflareListenersSetup) return

    solflare.on('connect', () => {
      if (isLocal.value) return
      walletType.value = 'external'
      svmAddress.value = solflare.publicKey?.toString() ?? null
    })

    solflare.on('disconnect', () => {
      if (!isLocal.value) {
        svmAddress.value = null
      }
    })

    solflareListenersSetup = true

    // Check if already connected
    if (solflare.isConnected && solflare.publicKey) {
      svmAddress.value = solflare.publicKey.toString()
      walletType.value = 'external'
    }
  }

  // Clear all wallet state
  function clearWalletState() {
    evmAddress.value = null
    svmAddress.value = null
    localEvmWallet.value = null
    localSvmKeypair.value = null
    error.value = null
  }

  // Initialize local wallets from environment variables
  async function initializeLocalWallets() {
    if (!isLocal.value) return

    error.value = null
    walletType.value = 'local'

    // Initialize EVM wallet
    const evmPrivateKey = import.meta.env.VITE_LOCAL_EVM_PRIVATE_KEY
    if (evmPrivateKey) {
      try {
        const rpcUrl = import.meta.env.VITE_ANVIL_RPC || 'http://localhost:8545'
        const provider = new JsonRpcProvider(rpcUrl)
        localEvmWallet.value = new Wallet(evmPrivateKey, provider)
        evmAddress.value = localEvmWallet.value.address
      } catch (e) {
        error.value = `Failed to initialize local EVM wallet: ${e instanceof Error ? e.message : 'Unknown error'}`
        console.error('Local EVM wallet error:', e)
      }
    }

    // Initialize SVM wallet
    const svmPrivateKey = import.meta.env.VITE_LOCAL_SVM_PRIVATE_KEY
    if (svmPrivateKey) {
      try {
        const secretKey = JSON.parse(svmPrivateKey)
        if (Array.isArray(secretKey)) {
          localSvmKeypair.value = Keypair.fromSecretKey(Uint8Array.from(secretKey))
          svmAddress.value = localSvmKeypair.value.publicKey.toBase58()
        }
      } catch (e) {
        error.value = `Failed to initialize local SVM wallet: ${e instanceof Error ? e.message : 'Unknown error'}`
        console.error('Local SVM wallet error:', e)
      }
    }
  }

  // Watch for network changes and reset wallet state
  watch(
    currentNetwork,
    async (newNetwork, oldNetwork) => {
      if (oldNetwork === undefined) {
        // Initial setup
        if (newNetwork === 'local') {
          await initializeLocalWallets()
        } else {
          setupWagmiWatcher()
          setupSolflareListeners()
        }
        return
      }

      // Network changed - clear existing wallet state
      clearWalletState()

      // Disconnect external wallets if switching away from devnet/mainnet
      if (oldNetwork !== 'local') {
        try {
          await wagmiDisconnect(wagmiConfig)
        } catch (e) {
          console.warn('Wagmi disconnect error:', e)
        }
        try {
          await solflare.disconnect()
        } catch (e) {
          console.warn('Solflare disconnect error:', e)
        }
      }

      // Setup for new network
      if (newNetwork === 'local') {
        await initializeLocalWallets()
      } else {
        setupWagmiWatcher()
        setupSolflareListeners()
      }
    },
    { immediate: true }
  )

  // Connect EVM wallet (browser extension via wagmi injected connector)
  async function connectEvm(): Promise<void> {
    error.value = null

    if (isLocal.value) {
      await initializeLocalWallets()
      return
    }

    try {
      const connectors = getConnectors(wagmiConfig)
      const injectedConnector = connectors.find(c => c.id === 'injected')

      if (!injectedConnector) {
        error.value = 'No browser wallet extension found. Please install Rabby or MetaMask.'
        return
      }

      await wagmiConnect(wagmiConfig, { connector: injectedConnector })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to connect EVM wallet'
      console.error('EVM connect error:', e)
    }
  }

  // Connect SVM wallet (Solflare)
  async function connectSvm(): Promise<void> {
    error.value = null

    if (isLocal.value) {
      await initializeLocalWallets()
      return
    }

    try {
      await solflare.connect()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to connect Solflare wallet'
      console.error('SVM connect error:', e)
    }
  }

  // Disconnect EVM wallet
  async function disconnectEvm(): Promise<void> {
    if (isLocal.value) {
      evmAddress.value = null
      localEvmWallet.value = null
      return
    }

    try {
      await wagmiDisconnect(wagmiConfig)
    } catch (e) {
      console.error('EVM disconnect error:', e)
    }
    evmAddress.value = null
  }

  // Disconnect SVM wallet
  async function disconnectSvm(): Promise<void> {
    if (isLocal.value) {
      svmAddress.value = null
      localSvmKeypair.value = null
      return
    }

    try {
      await solflare.disconnect()
    } catch (e) {
      console.error('SVM disconnect error:', e)
    }
    svmAddress.value = null
  }

  // Get EVM signer (for signing transactions)
  function getEvmSigner() {
    if (isLocal.value && localEvmWallet.value) {
      return localEvmWallet.value
    }
    // For external wallets, transactions are signed through the wallet extension
    return null
  }

  // Get SVM keypair (for signing transactions)
  function getSvmKeypair() {
    if (isLocal.value && localSvmKeypair.value) {
      return localSvmKeypair.value
    }
    // For external wallets, use solflare.signTransaction()
    return null
  }

  // Get Solflare instance for external wallet signing
  function getSolflare() {
    return isLocal.value ? null : solflare
  }

  // Cleanup on unmount
  onUnmounted(() => {
    unwatchWagmi?.()
  })

  return {
    // State
    evmAddress,
    svmAddress,
    evmConnected,
    svmConnected,
    isLocal,
    walletType,
    error,
    // Local wallet instances (for signing in local mode)
    localEvmWallet,
    localSvmKeypair,
    // Methods
    connectEvm,
    connectSvm,
    disconnectEvm,
    disconnectSvm,
    getEvmSigner,
    getSvmKeypair,
    getSolflare,
  }
}

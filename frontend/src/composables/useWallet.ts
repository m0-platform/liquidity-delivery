import { ref, computed, watch, toValue, type MaybeRef } from 'vue'
import { Wallet, JsonRpcProvider } from 'ethers'
import { Keypair } from '@solana/web3.js'
import { useAppKit, useAppKitAccount, useDisconnect, useAppKitNetwork } from '@reown/appkit/vue'
import { getAppKit, initializeAppKit } from '../appkit'

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

// EVM chain configurations
function getEvmChainConfig(network: NetworkType) {
  switch (network) {
    case 'local':
      return {
        chainId: 31337,
        chainIdHex: '0x7A69',
        chainName: 'Anvil Local',
        rpcUrl: import.meta.env.VITE_ANVIL_RPC || 'http://localhost:8545',
      }
    case 'devnet':
      return {
        chainId: 11155111,
        chainIdHex: '0xAA36A7',
        chainName: 'Sepolia',
        rpcUrl: 'https://sepolia.gateway.tenderly.co',
      }
    case 'mainnet':
      return {
        chainId: 1,
        chainIdHex: '0x1',
        chainName: 'Ethereum',
        rpcUrl: 'https://eth.llamarpc.com',
      }
  }
}

// Solana RPC configurations
function getSolanaRpc(network: NetworkType): string {
  switch (network) {
    case 'local':
      return import.meta.env.VITE_SURFPOOL_RPC || 'http://localhost:8899'
    case 'devnet':
      return 'https://api.devnet.solana.com'
    case 'mainnet':
      return 'https://api.mainnet-beta.solana.com'
  }
}

// Parse SVM private key from env (JSON array of bytes)
function parseSvmPrivateKey(keyStr: string): Uint8Array | null {
  if (!keyStr) return null
  try {
    const parsed = JSON.parse(keyStr)
    if (Array.isArray(parsed)) {
      return Uint8Array.from(parsed)
    }
    return null
  } catch {
    return null
  }
}

export function useWallet(networkRef: MaybeRef<NetworkType>) {
  const evmAddress = ref<string | null>(null)
  const svmAddress = ref<string | null>(null)
  const error = ref<string | null>(null)
  const walletType = ref<'local' | 'external'>('external')

  // Local mode wallet instances (for signing)
  const localEvmWallet = ref<Wallet | null>(null)
  const localSvmKeypair = ref<Keypair | null>(null)

  // Computed that reactively gets the current network
  const currentNetwork = computed(() => toValue(networkRef))
  const isLocal = computed(() => currentNetwork.value === 'local')
  const evmConnected = computed(() => !!evmAddress.value)
  const svmConnected = computed(() => !!svmAddress.value)

  // AppKit hooks (initialized lazily for non-local mode)
  let appKitAccount: ReturnType<typeof useAppKitAccount> | null = null
  let appKitNetwork: ReturnType<typeof useAppKitNetwork> | null = null
  let appKitDisconnect: ReturnType<typeof useDisconnect> | null = null
  let appKitModal: ReturnType<typeof useAppKit> | null = null
  let appKitInitialized = false

  function ensureAppKitInitialized() {
    if (isLocal.value) return false

    // Initialize AppKit if not already done
    if (!getAppKit()) {
      initializeAppKit()
    }

    const appKit = getAppKit()
    if (appKit && !appKitInitialized) {
      try {
        appKitAccount = useAppKitAccount()
        appKitNetwork = useAppKitNetwork()
        appKitDisconnect = useDisconnect()
        appKitModal = useAppKit()
        appKitInitialized = true

        // Watch AppKit account changes
        if (appKitAccount && appKitNetwork) {
          watch(
            [() => appKitAccount?.value?.address, () => appKitNetwork?.value?.caipNetwork?.chainNamespace],
            ([newAddress, chainNamespace]) => {
              if (!isLocal.value && newAddress) {
                console.log('AppKit account update:', { newAddress, chainNamespace })

                // Use chainNamespace to determine wallet type, fallback to address format
                if (chainNamespace === 'solana' ||
                    (newAddress && !newAddress.startsWith('0x') && newAddress.length >= 32)) {
                  svmAddress.value = newAddress
                  walletType.value = 'external'
                } else if (chainNamespace === 'eip155' || newAddress.startsWith('0x')) {
                  evmAddress.value = newAddress
                  walletType.value = 'external'
                }
              }
            },
            { immediate: true }
          )

          // Watch for disconnection
          watch(
            () => appKitAccount?.value?.isConnected,
            (isConnected) => {
              if (!isLocal.value && isConnected === false) {
                // AppKit disconnected - clear addresses
                evmAddress.value = null
                svmAddress.value = null
              }
            }
          )
        }
        return true
      } catch (e) {
        console.warn('AppKit hooks not available:', e)
        return false
      }
    }
    return appKitInitialized
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
        const chainConfig = getEvmChainConfig(currentNetwork.value)
        const provider = new JsonRpcProvider(chainConfig.rpcUrl)
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
        const secretKey = parseSvmPrivateKey(svmPrivateKey)
        if (secretKey) {
          localSvmKeypair.value = Keypair.fromSecretKey(secretKey)
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
        }
        return
      }

      // Network changed - clear existing wallet state
      clearWalletState()

      // Disconnect AppKit if switching away from external wallet mode
      if (oldNetwork !== 'local' && appKitDisconnect) {
        try {
          await appKitDisconnect.disconnect()
        } catch (e) {
          console.warn('AppKit disconnect error:', e)
        }
      }

      // Auto-connect local wallets when switching to local mode
      if (newNetwork === 'local') {
        await initializeLocalWallets()
      }
    },
    { immediate: true }
  )

  // Connect wallet (opens AppKit modal for non-local networks)
  async function connect(): Promise<void> {
    error.value = null

    if (isLocal.value) {
      await initializeLocalWallets()
      return
    }

    // Ensure AppKit is initialized for devnet/mainnet
    if (!ensureAppKitInitialized()) {
      error.value = 'AppKit not initialized. Please check your Reown project ID.'
      return
    }

    try {
      appKitModal!.open()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to open wallet modal'
    }
  }

  // Connect EVM specifically
  async function connectEvm(): Promise<void> {
    error.value = null

    if (isLocal.value) {
      await initializeLocalWallets()
      return
    }

    if (!ensureAppKitInitialized()) {
      error.value = 'AppKit not initialized. Please check your Reown project ID.'
      return
    }

    try {
      appKitModal!.open({ view: 'Connect' })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to open wallet modal'
    }
  }

  // Connect SVM specifically
  async function connectSvm(): Promise<void> {
    error.value = null

    if (isLocal.value) {
      await initializeLocalWallets()
      return
    }

    if (!ensureAppKitInitialized()) {
      error.value = 'AppKit not initialized. Please check your Reown project ID.'
      return
    }

    try {
      appKitModal!.open({ view: 'Connect' })
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to open wallet modal'
    }
  }

  // Disconnect wallet
  async function disconnect(): Promise<void> {
    if (isLocal.value) {
      clearWalletState()
      return
    }

    if (appKitDisconnect) {
      try {
        await appKitDisconnect.disconnect()
      } catch (e) {
        console.error('Disconnect error:', e)
      }
    }

    evmAddress.value = null
    svmAddress.value = null
  }

  // Disconnect EVM only
  async function disconnectEvm(): Promise<void> {
    if (isLocal.value) {
      evmAddress.value = null
      localEvmWallet.value = null
      return
    }

    // For external wallets, disconnect via AppKit
    if (appKitDisconnect) {
      try {
        await appKitDisconnect.disconnect()
      } catch (e) {
        console.error('EVM disconnect error:', e)
      }
    }
    evmAddress.value = null
  }

  // Disconnect SVM only
  async function disconnectSvm(): Promise<void> {
    if (isLocal.value) {
      svmAddress.value = null
      localSvmKeypair.value = null
      return
    }

    // For external wallets, disconnect via AppKit
    if (appKitDisconnect) {
      try {
        await appKitDisconnect.disconnect()
      } catch (e) {
        console.error('SVM disconnect error:', e)
      }
    }
    svmAddress.value = null
  }

  // Get EVM signer (for signing transactions)
  function getEvmSigner() {
    if (isLocal.value && localEvmWallet.value) {
      return localEvmWallet.value
    }
    // For AppKit, the signer would be obtained through wagmi
    return null
  }

  // Get SVM keypair (for signing transactions)
  function getSvmKeypair() {
    if (isLocal.value && localSvmKeypair.value) {
      return localSvmKeypair.value
    }
    // For AppKit, signing is handled through the Solana adapter
    return null
  }

  // Get chain config (reactive)
  const evmChainConfig = computed(() => getEvmChainConfig(currentNetwork.value))
  const solanaRpc = computed(() => getSolanaRpc(currentNetwork.value))

  return {
    // State
    evmAddress,
    svmAddress,
    evmConnected,
    svmConnected,
    isLocal,
    walletType,
    error,
    // Config
    evmChainConfig,
    solanaRpc,
    // Methods
    connect,
    connectEvm,
    connectSvm,
    disconnect,
    disconnectEvm,
    disconnectSvm,
    getEvmSigner,
    getSvmKeypair,
  }
}

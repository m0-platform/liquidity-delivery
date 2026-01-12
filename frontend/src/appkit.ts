import { createAppKit } from '@reown/appkit/vue'
import { WagmiAdapter } from '@reown/appkit-adapter-wagmi'
import { SolanaAdapter } from '@reown/appkit-adapter-solana'
import { mainnet, sepolia } from '@reown/appkit/networks'
import type { AppKitNetwork } from '@reown/appkit/networks'

// Define Solana networks manually since they may not be exported
const solanaMainnet: AppKitNetwork = {
  id: 'solana:mainnet',
  name: 'Solana',
  chainNamespace: 'solana',
  caipNetworkId: 'solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp',
  rpcUrls: {
    default: {
      http: ['https://api.mainnet-beta.solana.com'],
    },
  },
  nativeCurrency: {
    name: 'Solana',
    symbol: 'SOL',
    decimals: 9,
  },
}

const solanaDevnet: AppKitNetwork = {
  id: 'solana:devnet',
  name: 'Solana Devnet',
  chainNamespace: 'solana',
  caipNetworkId: 'solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1',
  rpcUrls: {
    default: {
      http: ['https://api.devnet.solana.com'],
    },
  },
  nativeCurrency: {
    name: 'Solana',
    symbol: 'SOL',
    decimals: 9,
  },
}

// Get project ID from environment
const projectId = import.meta.env.VITE_REOWN_PROJECT_ID || ''

// Define networks based on environment
const networks: [AppKitNetwork, ...AppKitNetwork[]] = [
  mainnet,
  sepolia,
  solanaMainnet,
  solanaDevnet,
]

// Metadata for the app
const metadata = {
  name: 'Liquidity Delivery',
  description: 'Cross-chain liquidity powered by M0',
  url: typeof window !== 'undefined' ? window.location.origin : 'https://localhost:5173',
  icons: ['https://avatars.githubusercontent.com/u/179229932'],
}

// Create adapters
const wagmiAdapter = new WagmiAdapter({
  projectId,
  networks,
})

const solanaAdapter = new SolanaAdapter()

// Create and export the AppKit instance
// Only initialize if we have a project ID (devnet/mainnet mode)
let appKit: ReturnType<typeof createAppKit> | null = null

export function initializeAppKit() {
  if (!projectId) {
    console.warn('No Reown project ID provided. AppKit will not be initialized.')
    return null
  }

  if (appKit) {
    return appKit
  }

  appKit = createAppKit({
    adapters: [wagmiAdapter, solanaAdapter],
    networks,
    metadata,
    projectId,
    features: {
      analytics: true,
      email: false,
      socials: false,
    },
  })

  return appKit
}

export function getAppKit() {
  return appKit
}

export { wagmiAdapter, solanaAdapter, networks }

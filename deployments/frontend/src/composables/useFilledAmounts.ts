import { ref } from 'vue'
import { JsonRpcProvider, Contract } from 'ethers'

// ABI for getFilledAmounts function from IOrderBook
const ORDERBOOK_ABI = [
  'function getFilledAmounts(bytes32 orderId_) external view returns (tuple(uint128 amountInReleased, uint128 amountOutFilled))'
]

export interface FilledAmounts {
  amountInReleased: string
  amountOutFilled: string
}

export interface ChainConfig {
  chainId: number
  name: string
  rpcUrl: string
  orderbookAddress: string
}

// Chain configurations with orderbook addresses
// These should match the quoter config
export function getChainConfigs(network: 'local' | 'devnet' | 'mainnet'): ChainConfig[] {
  if (network === 'local') {
    return [
      {
        chainId: 1,
        name: 'Ethereum (Anvil)',
        rpcUrl: 'http://localhost:8545',
        orderbookAddress: '0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9'
      },
      {
        chainId: 8453,
        name: 'Base (Anvil)',
        rpcUrl: 'http://localhost:8546',
        orderbookAddress: '0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9'
      }
    ]
  } else if (network === 'devnet') {
    return [
      {
        chainId: 11155111,
        name: 'Sepolia',
        rpcUrl: 'https://sepolia.gateway.tenderly.co',
        orderbookAddress: import.meta.env.VITE_SEPOLIA_ORDERBOOK_ADDRESS || ''
      },
      {
        chainId: 84532,
        name: 'Base Sepolia',
        rpcUrl: 'https://sepolia.base.org',
        orderbookAddress: import.meta.env.VITE_BASE_SEPOLIA_ORDERBOOK_ADDRESS || ''
      }
    ]
  } else {
    return [
      {
        chainId: 1,
        name: 'Ethereum',
        rpcUrl: 'https://eth.llamarpc.com',
        orderbookAddress: import.meta.env.VITE_ETHEREUM_ORDERBOOK_ADDRESS || ''
      },
      {
        chainId: 8453,
        name: 'Base',
        rpcUrl: 'https://mainnet.base.org',
        orderbookAddress: import.meta.env.VITE_BASE_ORDERBOOK_ADDRESS || ''
      },
      {
        chainId: 42161,
        name: 'Arbitrum',
        rpcUrl: 'https://arb1.arbitrum.io/rpc',
        orderbookAddress: import.meta.env.VITE_ARBITRUM_ORDERBOOK_ADDRESS || ''
      }
    ]
  }
}

export function useFilledAmounts() {
  const loading = ref(false)
  const error = ref<string | null>(null)
  const filledAmounts = ref<FilledAmounts | null>(null)

  async function fetchFilledAmounts(
    orderId: string,
    destChainId: number,
    network: 'local' | 'devnet' | 'mainnet'
  ): Promise<FilledAmounts | null> {
    loading.value = true
    error.value = null

    try {
      // Find chain config for destination chain
      const chainConfigs = getChainConfigs(network)
      const chainConfig = chainConfigs.find(c => c.chainId === destChainId)

      if (!chainConfig) {
        throw new Error(`Chain ${destChainId} not supported for filled amounts query`)
      }

      if (!chainConfig.orderbookAddress) {
        throw new Error(`No orderbook address configured for chain ${destChainId}`)
      }

      // Ensure orderId is properly formatted as bytes32
      const formattedOrderId = orderId.startsWith('0x') ? orderId : `0x${orderId}`

      const provider = new JsonRpcProvider(chainConfig.rpcUrl)
      const contract = new Contract(chainConfig.orderbookAddress, ORDERBOOK_ABI, provider)

      const result = await contract.getFilledAmounts(formattedOrderId)

      filledAmounts.value = {
        amountInReleased: result.amountInReleased.toString(),
        amountOutFilled: result.amountOutFilled.toString()
      }

      return filledAmounts.value
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Failed to fetch filled amounts'
      console.error('Filled amounts fetch error:', err)
      return null
    } finally {
      loading.value = false
    }
  }

  function clearFilledAmounts() {
    filledAmounts.value = null
    error.value = null
  }

  return {
    loading,
    error,
    filledAmounts,
    fetchFilledAmounts,
    clearFilledAmounts
  }
}

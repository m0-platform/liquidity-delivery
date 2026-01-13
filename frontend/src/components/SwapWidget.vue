<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import { useQuoter } from '../composables/useQuoter'
import { useAssets } from '../composables/useAssets'
import { useBalance } from '../composables/useBalance'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
  connected: boolean
  evmAddress: string | null
  svmAddress: string | null
}>()

const { getQuote, loading, error, quote } = useQuoter()
const { assets } = useAssets()
const { fetchBalance, loading: balanceLoading } = useBalance()

// Form state
const fromChain = ref('ethereum')
const toChain = ref('solana')
const fromToken = ref('USDC')
const toToken = ref('USDC')
const amount = ref('')

// Debounce state
let debounceTimer: ReturnType<typeof setTimeout> | null = null
const DEBOUNCE_DELAY = 500

// Balance state
const fromBalance = ref<string | null>(null)
const toBalance = ref<string | null>(null)
const fromBalanceLoading = ref(false)
const toBalanceLoading = ref(false)
const fromBalanceHovered = ref(false)
const toBalanceHovered = ref(false)

// Chain options based on network
const chains = computed(() => {
  if (props.network === 'local') {
    return [
      { id: 'ethereum', name: 'Ethereum (Anvil)', chainId: 1, rpc: 'http://localhost:8545', color: 'ethereum' },
      { id: 'base', name: 'Base (Anvil)', chainId: 8453, rpc: 'http://localhost:8546', color: 'base' },
      { id: 'solana', name: 'Solana (Surfpool)', chainId: 1399811149, rpc: 'http://localhost:8899', color: 'solana' },
    ]
  } else if (props.network === 'devnet') {
    return [
      { id: 'ethereum', name: 'Sepolia', chainId: 11155111, rpc: 'https://sepolia.gateway.tenderly.co', color: 'ethereum' },
      { id: 'base', name: 'Base Sepolia', chainId: 84532, rpc: 'https://sepolia.base.org', color: 'base' },
      { id: 'solana', name: 'Solana Devnet', chainId: 1399811150, rpc: 'https://api.devnet.solana.com', color: 'solana' },
    ]
  } else {
    return [
      { id: 'ethereum', name: 'Ethereum', chainId: 1, rpc: 'https://eth.llamarpc.com', color: 'ethereum' },
      { id: 'base', name: 'Base', chainId: 8453, rpc: 'https://mainnet.base.org', color: 'base' },
      { id: 'arbitrum', name: 'Arbitrum', chainId: 42161, rpc: 'https://arb1.arbitrum.io/rpc', color: 'arbitrum' },
      { id: 'solana', name: 'Solana', chainId: 1399811149, rpc: 'https://api.mainnet-beta.solana.com', color: 'solana' },
    ]
  }
})

// Get tokens from API assets, fallback to static list if API hasn't loaded
const tokens = computed(() => {
  if (assets.value.length > 0) {
    return assets.value.map(asset => ({
      ticker: asset.ticker,
      name: asset.name,
      icon: asset.icon,
      address: asset.address,
      decimals: asset.decimals,
    }))
  }
  return [
    { ticker: 'USDC', name: 'USD Coin', icon: '', address: '', decimals: 6 },
    { ticker: 'USDT', name: 'Tether USD', icon: '', address: '', decimals: 6 },
    { ticker: 'M', name: 'M Token', icon: '', address: '', decimals: 6 },
  ]
})

// Get asset by ticker
function getAsset(ticker: string) {
  return tokens.value.find(t => t.ticker === ticker)
}

// Convert float amount to integer based on decimals
function toIntegerAmount(amount: string, decimals: number): string {
  const num = parseFloat(amount)
  if (isNaN(num)) return '0'
  const shifted = num * Math.pow(10, decimals)
  return Math.floor(shifted).toString()
}

// Get token icon URL from assets
function getTokenIcon(ticker: string): string | null {
  const token = tokens.value.find(t => t.ticker === ticker)
  return token?.icon || null
}

// Chain icon component
function getChainColor(chainId: string): string {
  const colorMap: Record<string, string> = {
    'ethereum': '#627eea',
    'solana': '#9945ff',
    'base': '#0052ff',
    'arbitrum': '#28a0f0',
  }
  return colorMap[chainId] || '#64748b'
}

// Swap direction
function swapDirection() {
  const tempChain = fromChain.value
  fromChain.value = toChain.value
  toChain.value = tempChain

  const tempToken = fromToken.value
  fromToken.value = toToken.value
  toToken.value = tempToken
}

// Request quote with debouncing
async function requestQuote() {
  const srcChain = chains.value.find(c => c.id === fromChain.value)
  const dstChain = chains.value.find(c => c.id === toChain.value)
  const srcAsset = getAsset(fromToken.value)
  const dstAsset = getAsset(toToken.value)

  if (!srcChain || !dstChain || !amount.value || !srcAsset || !dstAsset) return

  // Convert float amount to integer using decimal shift
  const integerAmount = toIntegerAmount(amount.value, srcAsset.decimals)

  await getQuote({
    srcChainId: srcChain.chainId,
    dstChainId: dstChain.chainId,
    srcToken: srcAsset.address,
    dstToken: dstAsset.address,
    amount: integerAmount,
  })
}

// Debounced quote request
function debouncedRequestQuote() {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
  }

  if (amount.value && parseFloat(amount.value) > 0) {
    debounceTimer = setTimeout(() => {
      requestQuote()
    }, DEBOUNCE_DELAY)
  }
}

// Watch for input changes and trigger debounced quote
watch([fromChain, toChain, fromToken, toToken, amount], () => {
  debouncedRequestQuote()
})

// Cleanup debounce timer on unmount
onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
  }
})

// Format rate display
function formatRate(rate: number): string {
  if (!rate || rate === 0) return '—'
  return rate.toFixed(6)
}

// Format amount with commas
function formatAmount(val: string): string {
  const num = parseFloat(val)
  if (isNaN(num)) return '0.00'
  return num.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 6 })
}

// Get wallet address for a given chain
function getWalletAddressForChain(chainId: string): string | null {
  if (chainId === 'solana') {
    return props.svmAddress
  }
  return props.evmAddress
}

// Check if token address format is valid for the chain type
function isAddressValidForChain(address: string, chainId: string): boolean {
  if (!address) return false

  const isEvmAddress = address.startsWith('0x')
  const isSolanaChain = chainId === 'solana'

  // EVM addresses start with 0x, Solana addresses are base58 (no 0x prefix)
  if (isSolanaChain && isEvmAddress) {
    return false // EVM address on Solana chain - invalid
  }
  if (!isSolanaChain && !isEvmAddress) {
    return false // Solana address on EVM chain - invalid
  }
  return true
}

// Fetch balance for the "from" side
async function fetchFromBalance() {
  const chain = chains.value.find(c => c.id === fromChain.value)
  const asset = getAsset(fromToken.value)
  const walletAddress = getWalletAddressForChain(fromChain.value)

  // Check if address format is compatible with chain
  const addressValid = asset?.address ? isAddressValidForChain(asset.address, fromChain.value) : false

  if (!chain || !asset || !walletAddress || !asset.address || !addressValid) {
    fromBalance.value = null
    return
  }

  fromBalanceLoading.value = true
  try {
    const result = await fetchBalance(
      fromChain.value,
      chain.rpc,
      walletAddress,
      asset.address,
      asset.decimals
    )
    fromBalance.value = result?.formatted || null
  } catch (err) {
    console.error('Failed to fetch from balance:', err)
    fromBalance.value = null
  } finally {
    fromBalanceLoading.value = false
  }
}

// Fetch balance for the "to" side
async function fetchToBalance() {
  const chain = chains.value.find(c => c.id === toChain.value)
  const asset = getAsset(toToken.value)
  const walletAddress = getWalletAddressForChain(toChain.value)

  // Check if address format is compatible with chain
  const addressValid = asset?.address ? isAddressValidForChain(asset.address, toChain.value) : false

  if (!chain || !asset || !walletAddress || !asset.address || !addressValid) {
    toBalance.value = null
    return
  }

  toBalanceLoading.value = true
  try {
    const result = await fetchBalance(
      toChain.value,
      chain.rpc,
      walletAddress,
      asset.address,
      asset.decimals
    )
    toBalance.value = result?.formatted || null
  } catch (err) {
    console.error('Failed to fetch to balance:', err)
    toBalance.value = null
  } finally {
    toBalanceLoading.value = false
  }
}

// Watch for changes that should trigger balance refresh
// Note: assets is included so balances are fetched once token addresses load from API
watch(
  [fromChain, fromToken, () => props.evmAddress, () => props.svmAddress, () => props.connected, assets],
  () => {
    if (props.connected) {
      fetchFromBalance()
    } else {
      fromBalance.value = null
    }
  },
  { immediate: true }
)

watch(
  [toChain, toToken, () => props.evmAddress, () => props.svmAddress, () => props.connected, assets],
  () => {
    if (props.connected) {
      fetchToBalance()
    } else {
      toBalance.value = null
    }
  },
  { immediate: true }
)

// Set max amount from balance
function setMaxFromBalance() {
  if (fromBalance.value) {
    // Remove commas and set as amount
    amount.value = fromBalance.value.replace(/,/g, '')
  }
}
</script>

<template>
  <div class="glass-card rounded-3xl p-6">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-xl font-semibold text-white">Swap</h2>
      <button class="btn-ghost p-2 rounded-lg">
        <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>

    <!-- From Section -->
    <div class="bg-slate-925/60 rounded-2xl p-4 mb-2 border border-white/5 transition-all duration-200 focus-within:border-accent-500/30">
      <div class="flex justify-between items-center mb-3">
        <span class="text-sm text-surface-400">You pay</span>
        <span class="text-sm text-surface-500 flex items-center gap-1">
          Balance:
          <button
            v-if="connected"
            @click="fetchFromBalance"
            @mouseenter="fromBalanceHovered = true"
            @mouseleave="fromBalanceHovered = false"
            class="relative group text-surface-300 hover:text-accent-400 transition-colors"
            :class="{ 'cursor-pointer': !fromBalanceLoading }"
            :disabled="fromBalanceLoading"
            :title="fromBalanceHovered ? 'Click to refresh balance' : ''"
          >
            <span v-if="fromBalanceLoading" class="flex items-center gap-1">
              <svg class="w-3 h-3 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2v4m0 12v4m10-10h-4M6 12H2m15.364-6.364l-2.828 2.828M9.464 14.536l-2.828 2.828m12.728 0l-2.828-2.828M9.464 9.464L6.636 6.636" stroke-linecap="round"/>
              </svg>
            </span>
            <span v-else class="flex items-center gap-1">
              {{ fromBalance || '—' }}
              <svg
                v-if="fromBalanceHovered"
                class="w-3 h-3 text-accent-400"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </span>
            <span
              v-if="fromBalanceHovered && !fromBalanceLoading"
              class="absolute -top-8 left-1/2 -translate-x-1/2 px-2 py-1 bg-slate-800 text-xs text-surface-300 rounded whitespace-nowrap z-10"
            >
              Click to refresh
            </span>
          </button>
          <span v-else class="text-surface-300">—</span>
          <button
            v-if="connected && fromBalance"
            @click="setMaxFromBalance"
            class="ml-1 text-xs text-accent-500 hover:text-accent-400 transition-colors font-medium"
          >
            MAX
          </button>
        </span>
      </div>

      <div class="flex items-center gap-4">
        <!-- Amount Input -->
        <input
          v-model="amount"
          type="number"
          placeholder="0.00"
          class="flex-1 bg-transparent text-3xl font-medium text-white outline-none placeholder-surface-600 min-w-0"
        />

        <!-- Token & Chain Selectors -->
        <div class="flex flex-col gap-2">
          <!-- Token Selector -->
          <div class="flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-accent-500/30 transition-colors cursor-pointer">
            <img
              v-if="getTokenIcon(fromToken)"
              :src="getTokenIcon(fromToken)!"
              :alt="fromToken"
              class="w-6 h-6 rounded-full"
            />
            <div v-else class="token-icon token-icon-default w-6 h-6 text-[10px]">
              {{ fromToken.charAt(0) }}
            </div>
            <select
              v-model="fromToken"
              class="bg-transparent text-sm font-medium text-white outline-none cursor-pointer appearance-none pr-4"
            >
              <option v-for="token in tokens" :key="token.ticker" :value="token.ticker" class="bg-slate-900">
                {{ token.ticker }}
              </option>
            </select>
          </div>

          <!-- Chain Selector -->
          <select
            v-model="fromChain"
            class="select-field text-xs py-1.5 px-2"
          >
            <option v-for="chain in chains" :key="chain.id" :value="chain.id" class="bg-slate-900">
              {{ chain.name }}
            </option>
          </select>
        </div>
      </div>
    </div>

    <!-- Swap Direction Button -->
    <div class="flex justify-center -my-3 relative z-10">
      <button
        @click="swapDirection"
        class="swap-direction-btn"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4" />
        </svg>
      </button>
    </div>

    <!-- To Section -->
    <div class="bg-slate-925/60 rounded-2xl p-4 mb-4 border border-white/5">
      <div class="flex justify-between items-center mb-3">
        <span class="text-sm text-surface-400">You receive</span>
        <span class="text-sm text-surface-500 flex items-center gap-1">
          Balance:
          <button
            v-if="connected"
            @click="fetchToBalance"
            @mouseenter="toBalanceHovered = true"
            @mouseleave="toBalanceHovered = false"
            class="relative group text-surface-300 hover:text-accent-400 transition-colors"
            :class="{ 'cursor-pointer': !toBalanceLoading }"
            :disabled="toBalanceLoading"
            :title="toBalanceHovered ? 'Click to refresh balance' : ''"
          >
            <span v-if="toBalanceLoading" class="flex items-center gap-1">
              <svg class="w-3 h-3 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2v4m0 12v4m10-10h-4M6 12H2m15.364-6.364l-2.828 2.828M9.464 14.536l-2.828 2.828m12.728 0l-2.828-2.828M9.464 9.464L6.636 6.636" stroke-linecap="round"/>
              </svg>
            </span>
            <span v-else class="flex items-center gap-1">
              {{ toBalance || '—' }}
              <svg
                v-if="toBalanceHovered"
                class="w-3 h-3 text-accent-400"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </span>
            <span
              v-if="toBalanceHovered && !toBalanceLoading"
              class="absolute -top-8 left-1/2 -translate-x-1/2 px-2 py-1 bg-slate-800 text-xs text-surface-300 rounded whitespace-nowrap z-10"
            >
              Click to refresh
            </span>
          </button>
          <span v-else class="text-surface-300">—</span>
        </span>
      </div>

      <div class="flex items-center gap-4">
        <!-- Output Amount -->
        <div class="flex-1 min-w-0">
          <div v-if="loading" class="flex items-center gap-3">
            <div class="loading-spinner"></div>
            <span class="text-surface-400 text-lg">Fetching quote...</span>
          </div>
          <div v-else class="text-3xl font-medium" :class="quote?.amountOut ? 'text-white' : 'text-surface-600'">
            {{ quote?.amountOut ? formatAmount(quote.amountOut) : '0.00' }}
          </div>
        </div>

        <!-- Token & Chain Selectors -->
        <div class="flex flex-col gap-2">
          <!-- Token Selector -->
          <div class="flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-accent-500/30 transition-colors cursor-pointer">
            <img
              v-if="getTokenIcon(toToken)"
              :src="getTokenIcon(toToken)!"
              :alt="toToken"
              class="w-6 h-6 rounded-full"
            />
            <div v-else class="token-icon token-icon-default w-6 h-6 text-[10px]">
              {{ toToken.charAt(0) }}
            </div>
            <select
              v-model="toToken"
              class="bg-transparent text-sm font-medium text-white outline-none cursor-pointer appearance-none pr-4"
            >
              <option v-for="token in tokens" :key="token.ticker" :value="token.ticker" class="bg-slate-900">
                {{ token.ticker }}
              </option>
            </select>
          </div>

          <!-- Chain Selector -->
          <select
            v-model="toChain"
            class="select-field text-xs py-1.5 px-2"
          >
            <option v-for="chain in chains" :key="chain.id" :value="chain.id" class="bg-slate-900">
              {{ chain.name }}
            </option>
          </select>
        </div>
      </div>
    </div>

    <!-- Loading Bar -->
    <div v-if="loading" class="mb-4">
      <div class="loading-liquid"></div>
    </div>

    <!-- Quote Details -->
    <Transition
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="opacity-0 -translate-y-2"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div v-if="quote && !loading" class="bg-slate-925/40 rounded-xl p-4 mb-4 border border-white/5">
        <div class="space-y-0">
          <!-- Rate -->
          <div class="quote-row">
            <span class="quote-label flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Rate
            </span>
            <span class="quote-value">
              1 {{ fromToken }} = <span class="quote-highlight">{{ formatRate(quote.rate) }}</span> {{ toToken }}
            </span>
          </div>

          <!-- Fee -->
          <div class="quote-row">
            <span class="quote-label flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Fee
            </span>
            <span class="quote-value">{{ quote.fee }} bps</span>
          </div>

          <!-- Estimated Time -->
          <div class="quote-row">
            <span class="quote-label flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Est. Time
            </span>
            <span class="quote-value text-accent-400">{{ quote.estimatedTime }}</span>
          </div>

          <!-- Solver -->
          <div v-if="quote.solver" class="quote-row border-b-0">
            <span class="quote-label flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Solver
            </span>
            <span class="quote-value text-xs font-mono text-surface-400">
              {{ quote.solver.slice(0, 8) }}...{{ quote.solver.slice(-6) }}
            </span>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Error State -->
    <Transition
      enter-active-class="transition-all duration-300 ease-out"
      enter-from-class="opacity-0 scale-95"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition-all duration-200 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <div v-if="error" class="bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 mb-4 flex items-start gap-3">
        <svg class="w-5 h-5 flex-shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span class="text-sm">{{ error }}</span>
      </div>
    </Transition>

    <!-- Swap Button -->
    <button
      :disabled="!connected || !quote || loading"
      :class="[
        'btn-primary w-full text-lg',
        !connected && 'opacity-60'
      ]"
    >
      <span v-if="!connected" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Connect Wallet
      </span>
      <span v-else-if="loading" class="flex items-center justify-center gap-2">
        <div class="loading-spinner"></div>
        Getting Quote...
      </span>
      <span v-else>Swap</span>
    </button>

    <!-- Route Info -->
    <div v-if="quote && !loading" class="mt-4 pt-4 border-t border-white/5">
      <div class="flex items-center justify-center gap-2 text-xs text-surface-500">
        <span class="flex items-center gap-1">
          <div class="w-2 h-2 rounded-full" :style="{ backgroundColor: getChainColor(fromChain) }"></div>
          {{ chains.find(c => c.id === fromChain)?.name }}
        </span>
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span class="flex items-center gap-1">
          <div class="w-2 h-2 rounded-full" :style="{ backgroundColor: getChainColor(toChain) }"></div>
          {{ chains.find(c => c.id === toChain)?.name }}
        </span>
      </div>
    </div>
  </div>
</template>

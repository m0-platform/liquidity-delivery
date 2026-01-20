<script setup lang="ts">
import { ref, computed, watch, onUnmounted, toRef } from 'vue'
import { useQuoter } from '../composables/useQuoter'
import { useAssets } from '../composables/useAssets'
import { useBalance } from '../composables/useBalance'
import { useSwap, type ChainType } from '../composables/useSwap'
import { getNetworkConfig } from '../config/network'
import type { Wallet } from 'ethers'
import type { Keypair } from '@solana/web3.js'
import type Solflare from '@solflare-wallet/sdk'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
  connected: boolean
  evmAddress: string | null
  svmAddress: string | null
  evmSigner?: Wallet | null
  svmKeypair?: Keypair | null
  solflareWallet?: Solflare | null
}>()

const emit = defineEmits<{
  'order-created': [orderId: string]
}>()

const { getQuote, loading, error, quote } = useQuoter(toRef(props, 'network'))
const { assets, getAssetForChain, getTickersForChain } = useAssets(toRef(props, 'network'))
const { fetchBalance } = useBalance()
const { executeSwap, loading: swapLoading, error: swapError } = useSwap()

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

// Post-swap redirect loading state
const redirecting = ref(false)

// Chain options based on network
const chains = computed(() => {
  const config = getNetworkConfig(props.network)

  if (props.network === 'local') {
    return [
      { id: 'ethereum', name: 'Ethereum (Anvil)', chainId: 1, rpc: config.ethereumRpc },
      { id: 'base', name: 'Base (Anvil)', chainId: 8453, rpc: config.baseRpc || 'http://localhost:8546' },
      { id: 'solana', name: 'Solana (Surfpool)', chainId: 1399811149, rpc: config.solanaRpc },
    ]
  } else if (props.network === 'devnet') {
    return [
      { id: 'ethereum', name: 'Sepolia', chainId: 11155111, rpc: config.ethereumRpc },
      { id: 'solana', name: 'Solana Devnet', chainId: 1399811150, rpc: config.solanaRpc },
    ]
  } else {
    return [
      { id: 'ethereum', name: 'Ethereum', chainId: 1, rpc: config.ethereumRpc },
      { id: 'solana', name: 'Solana', chainId: 1399811149, rpc: config.solanaRpc },
    ]
  }
})

// Get token tickers available on the "from" chain
const fromTokenTickers = computed(() => {
  const chain = chains.value.find(c => c.id === fromChain.value)
  if (chain && assets.value.length > 0) {
    const tickers = getTickersForChain(chain.chainId)
    return tickers.length > 0 ? tickers : ['USDC', 'wM']
  }
  return ['USDC', 'wM']
})

// Get token tickers available on the "to" chain
const toTokenTickers = computed(() => {
  const chain = chains.value.find(c => c.id === toChain.value)
  if (chain && assets.value.length > 0) {
    const tickers = getTickersForChain(chain.chainId)
    return tickers.length > 0 ? tickers : ['USDC', 'wM']
  }
  return ['USDC', 'wM']
})

// Get a representative token info for display (icon, name) - uses first matching asset
function getTokenInfo(ticker: string) {
  const asset = assets.value.find(a => a.ticker === ticker)
  return asset ? { name: asset.name, icon: asset.icon } : { name: ticker, icon: '' }
}

// Get asset for a specific chain (returns address, decimals for that chain)
function getAssetForSelectedChain(ticker: string, chainId: string) {
  const chain = chains.value.find(c => c.id === chainId)
  if (!chain) return null
  return getAssetForChain(ticker, chain.chainId)
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
  const info = getTokenInfo(ticker)
  return info.icon || null
}

// Check if a chain ID represents a Solana chain
function isSolanaChain(chainId: string): boolean {
  return chainId === 'solana'
}

// Determine which wallets are required for the current swap configuration
const sourceRuntime = computed(() => isSolanaChain(fromChain.value) ? 'svm' : 'evm')
const destRuntime = computed(() => isSolanaChain(toChain.value) ? 'svm' : 'evm')

// Check which wallets are needed and connected
const needsEvmWallet = computed(() => sourceRuntime.value === 'evm' || destRuntime.value === 'evm')
const needsSvmWallet = computed(() => sourceRuntime.value === 'svm' || destRuntime.value === 'svm')

const hasRequiredEvmWallet = computed(() => !needsEvmWallet.value || !!props.evmAddress)
const hasRequiredSvmWallet = computed(() => !needsSvmWallet.value || !!props.svmAddress)

// Check if all required wallets are connected
const hasAllRequiredWallets = computed(() => hasRequiredEvmWallet.value && hasRequiredSvmWallet.value)

// Get the missing wallet type for display
const missingWalletType = computed(() => {
  if (!hasRequiredEvmWallet.value && !hasRequiredSvmWallet.value) return 'both'
  if (!hasRequiredEvmWallet.value) return 'evm'
  if (!hasRequiredSvmWallet.value) return 'svm'
  return null
})

// Reset token selection when chain changes or assets load if current token isn't available
watch([fromChain, fromTokenTickers], () => {
  if (!fromTokenTickers.value.includes(fromToken.value) && fromTokenTickers.value.length > 0) {
    fromToken.value = fromTokenTickers.value[0]
  }
})

watch([toChain, toTokenTickers], () => {
  if (!toTokenTickers.value.includes(toToken.value) && toTokenTickers.value.length > 0) {
    toToken.value = toTokenTickers.value[0]
  }
})

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
  const srcAsset = getAssetForSelectedChain(fromToken.value, fromChain.value)
  const dstAsset = getAssetForSelectedChain(toToken.value, toChain.value)

  if (!srcChain || !dstChain || !amount.value || !srcAsset || !dstAsset) return

  // Convert float amount to integer using decimal shift
  const integerAmount = toIntegerAmount(amount.value, srcAsset.decimals)

  // Get sender address based on source chain type
  const senderAddress = isSolanaChain(fromChain.value)
    ? props.svmAddress
    : props.evmAddress

  // Get recipient address based on destination chain type
  const recipientAddress = isSolanaChain(toChain.value)
    ? props.svmAddress
    : props.evmAddress

  await getQuote({
    srcChainId: srcChain.chainId,
    dstChainId: dstChain.chainId,
    srcToken: srcAsset.address,
    dstToken: dstAsset.address,
    amount: integerAmount,
    senderAddress: senderAddress || undefined,
    recipient: recipientAddress || undefined,
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
// Note: assets and network are included so quotes are fetched once token addresses load from API (on network change)
watch([fromChain, toChain, fromToken, toToken, amount, assets, () => props.network], () => {
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

// Convert raw integer amount to decimal string using token decimals
function fromIntegerAmount(rawAmount: string, decimals: number): string {
  if (!rawAmount || rawAmount === '0') return '0.00'
  const paddedRaw = rawAmount.padStart(decimals + 1, '0')
  const integerPart = paddedRaw.slice(0, -decimals) || '0'
  const decimalPart = paddedRaw.slice(-decimals)
  const trimmedDecimal = decimalPart.replace(/0+$/, '').padEnd(2, '0').slice(0, 6)
  return `${integerPart}.${trimmedDecimal}`
}

// Computed formatted output amount using destination token decimals
const formattedOutputAmount = computed(() => {
  if (!quote.value?.amountOut) return '0.00'
  const dstAsset = getAssetForSelectedChain(toToken.value, toChain.value)
  if (!dstAsset) return '0.00'
  const decimalAmount = fromIntegerAmount(quote.value.amountOut, dstAsset.decimals)
  return formatAmount(decimalAmount)
})

// Get wallet address for a given chain
function getWalletAddressForChain(chainId: string): string | null {
  if (chainId === 'solana') {
    return props.svmAddress
  }
  return props.evmAddress
}

// Fetch balance for the "from" side
async function fetchFromBalance() {
  const chain = chains.value.find(c => c.id === fromChain.value)
  const asset = getAssetForSelectedChain(fromToken.value, fromChain.value)
  const walletAddress = getWalletAddressForChain(fromChain.value)

  if (!chain || !asset || !walletAddress || !asset.address) {
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
      asset.decimals,
      asset.extensionTokenProgramId
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
  const asset = getAssetForSelectedChain(toToken.value, toChain.value)
  const walletAddress = getWalletAddressForChain(toChain.value)

  if (!chain || !asset || !walletAddress || !asset.address) {
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
      asset.decimals,
      asset.extensionTokenProgramId
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
// Note: assets and network are included so balances are fetched once token addresses load from API
watch(
  [fromChain, fromToken, () => props.evmAddress, () => props.svmAddress, () => props.connected, assets, () => props.network],
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
  [toChain, toToken, () => props.evmAddress, () => props.svmAddress, () => props.connected, assets, () => props.network],
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

// Handle swap execution
async function handleSwap() {
  if (!quote.value || !quote.value.orderId) {
    console.error('No quote or order ID available')
    return
  }

  const srcChain = chains.value.find(c => c.id === fromChain.value)
  if (!srcChain) {
    console.error('Source chain not found')
    return
  }

  try {
    const chainType: ChainType = isSolanaChain(fromChain.value) ? 'svm' : 'evm'

    const result = await executeSwap(chainType, {
      evmTransaction: quote.value.evmTransaction,
      approvalTransaction: quote.value.approvalTransaction,
      svmTransaction: quote.value.svmTransaction,
      orderId: quote.value.orderId,
      svmRpcUrl: srcChain.rpc,
      localEvmSigner: props.evmSigner,
      localSvmKeypair: props.svmKeypair,
      solflareWallet: props.solflareWallet,
    })

    console.log('Swap executed:', result)
    if (result.approvalTxHash) {
      console.log('Approval tx:', result.approvalTxHash)
    }

    redirecting.value = true
    await new Promise(resolve => setTimeout(resolve, 1000))

    // Emit order created event to navigate to order details
    emit('order-created', result.orderId)
  } catch (err) {
    console.error('Swap failed:', err)
  } finally {
    redirecting.value = false
  }
}

// Check if approval is needed
const needsApproval = computed(() => !!quote.value?.approvalTransaction)

// Combined loading state
const isLoading = computed(() => loading.value || swapLoading.value || redirecting.value)

// Combined error state
const displayError = computed(() => error.value || swapError.value)
</script>

<template>
  <div class="glass-card rounded-3xl p-6">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-xl font-semibold text-white">Swap</h2>
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
              <option v-for="ticker in fromTokenTickers" :key="ticker" :value="ticker" class="bg-slate-900">
                {{ ticker }}
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
            {{ formattedOutputAmount }}
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
              <option v-for="ticker in toTokenTickers" :key="ticker" :value="ticker" class="bg-slate-900">
                {{ ticker }}
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
      <div v-if="displayError" class="bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 mb-4 flex items-start gap-3">
        <svg class="w-5 h-5 flex-shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span class="text-sm">{{ displayError }}</span>
      </div>
    </Transition>

    <!-- Swap Button -->
    <button
      @click="handleSwap"
      :disabled="!hasAllRequiredWallets || !quote || isLoading"
      :class="[
        'btn-primary w-full text-lg',
        !hasAllRequiredWallets && 'opacity-60'
      ]"
    >
      <span v-if="!connected" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Connect Wallet
      </span>
      <span v-else-if="missingWalletType === 'both'" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Connect EVM &amp; Solana Wallets
      </span>
      <span v-else-if="missingWalletType === 'evm'" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Connect EVM Wallet
      </span>
      <span v-else-if="missingWalletType === 'svm'" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Connect Solana Wallet
      </span>
      <span v-else-if="loading" class="flex items-center justify-center gap-2">
        <div class="loading-spinner"></div>
        Getting Quote...
      </span>
      <span v-else-if="swapLoading" class="flex items-center justify-center gap-2">
        <div class="loading-spinner"></div>
        Executing Swap...
      </span>
      <span v-else-if="redirecting" class="flex items-center justify-center gap-2">
        <div class="loading-spinner"></div>
        Redirecting...
      </span>
      <span v-else-if="needsApproval" class="flex items-center justify-center gap-2">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Approve &amp; Swap
      </span>
      <span v-else>Swap</span>
    </button>

  </div>
</template>

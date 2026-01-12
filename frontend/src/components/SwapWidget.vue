<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useQuoter } from '../composables/useQuoter'
import { useAssets } from '../composables/useAssets'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
  connected: boolean
}>()

const { getQuote, loading, error, quote } = useQuoter()
const { assets } = useAssets()

// Form state
const fromChain = ref('ethereum')
const toChain = ref('solana')
const fromToken = ref('USDC')
const toToken = ref('USDC')
const amount = ref('')

// Chain options based on network
const chains = computed(() => {
  if (props.network === 'local') {
    return [
      { id: 'ethereum', name: 'Anvil (Local EVM)', chainId: 31337 },
      { id: 'solana', name: 'Surfpool (Local Solana)', chainId: 1399811149 },
    ]
  } else if (props.network === 'devnet') {
    return [
      { id: 'ethereum', name: 'Sepolia', chainId: 11155111 },
      { id: 'base', name: 'Base Sepolia', chainId: 84532 },
      { id: 'solana', name: 'Solana Devnet', chainId: 1399811150 },
    ]
  } else {
    return [
      { id: 'ethereum', name: 'Ethereum', chainId: 1 },
      { id: 'base', name: 'Base', chainId: 8453 },
      { id: 'arbitrum', name: 'Arbitrum', chainId: 42161 },
      { id: 'solana', name: 'Solana', chainId: 1399811149 },
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
    }))
  }
  return [
    { ticker: 'USDC', name: 'USD Coin', icon: '' },
    { ticker: 'USDT', name: 'Tether USD', icon: '' },
    { ticker: 'M', name: 'M Token', icon: '' },
  ]
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

// Request quote
async function requestQuote() {
  const srcChain = chains.value.find(c => c.id === fromChain.value)
  const dstChain = chains.value.find(c => c.id === toChain.value)

  if (!srcChain || !dstChain || !amount.value) return

  await getQuote({
    srcChainId: srcChain.chainId,
    dstChainId: dstChain.chainId,
    srcToken: fromToken.value,
    dstToken: toToken.value,
    amount: amount.value,
  })
}

// Auto-request quote when inputs change
watch([fromChain, toChain, fromToken, toToken, amount], () => {
  if (amount.value && parseFloat(amount.value) > 0) {
    requestQuote()
  }
}, { debounce: 500 } as any)
</script>

<template>
  <div class="bg-gray-800 rounded-2xl p-6 shadow-xl border border-gray-700">
    <h2 class="text-lg font-semibold mb-4">Swap</h2>

    <!-- From Section -->
    <div class="bg-gray-900 rounded-xl p-4 mb-2">
      <div class="flex justify-between text-sm text-gray-400 mb-2">
        <span>From</span>
        <span>Balance: --</span>
      </div>

      <div class="flex gap-3">
        <input
          v-model="amount"
          type="number"
          placeholder="0.0"
          class="flex-1 bg-transparent text-2xl font-medium outline-none"
        />

        <div class="flex gap-2">
          <select
            v-model="fromToken"
            class="bg-gray-700 rounded-lg px-3 py-2 text-sm"
          >
            <option v-for="token in tokens" :key="token.ticker" :value="token.ticker">
              {{ token.ticker }}
            </option>
          </select>

          <select
            v-model="fromChain"
            class="bg-gray-700 rounded-lg px-3 py-2 text-sm"
          >
            <option v-for="chain in chains" :key="chain.id" :value="chain.id">
              {{ chain.name }}
            </option>
          </select>
        </div>
      </div>
    </div>

    <!-- Swap Button -->
    <div class="flex justify-center -my-2 relative z-10">
      <button
        @click="swapDirection"
        class="bg-gray-700 hover:bg-gray-600 p-2 rounded-lg border-4 border-gray-800"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4" />
        </svg>
      </button>
    </div>

    <!-- To Section -->
    <div class="bg-gray-900 rounded-xl p-4 mb-4">
      <div class="flex justify-between text-sm text-gray-400 mb-2">
        <span>To</span>
        <span>Balance: --</span>
      </div>

      <div class="flex gap-3">
        <div class="flex-1 text-2xl font-medium text-gray-400">
          {{ quote?.amountOut || '0.0' }}
        </div>

        <div class="flex gap-2">
          <select
            v-model="toToken"
            class="bg-gray-700 rounded-lg px-3 py-2 text-sm"
          >
            <option v-for="token in tokens" :key="token.ticker" :value="token.ticker">
              {{ token.ticker }}
            </option>
          </select>

          <select
            v-model="toChain"
            class="bg-gray-700 rounded-lg px-3 py-2 text-sm"
          >
            <option v-for="chain in chains" :key="chain.id" :value="chain.id">
              {{ chain.name }}
            </option>
          </select>
        </div>
      </div>
    </div>

    <!-- Quote Details -->
    <div v-if="quote" class="bg-gray-900 rounded-xl p-4 mb-4 text-sm">
      <div class="flex justify-between text-gray-400 mb-1">
        <span>Rate</span>
        <span>1 {{ fromToken }} = {{ quote.rate }} {{ toToken }}</span>
      </div>
      <div class="flex justify-between text-gray-400 mb-1">
        <span>Fee</span>
        <span>{{ quote.fee }} {{ fromToken }}</span>
      </div>
      <div class="flex justify-between text-gray-400">
        <span>Est. Time</span>
        <span>~{{ quote.estimatedTime }}</span>
      </div>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="text-center text-gray-400 mb-4">
      Getting quote...
    </div>

    <!-- Error State -->
    <div v-if="error" class="bg-red-900/50 text-red-300 rounded-xl p-4 mb-4 text-sm">
      {{ error }}
    </div>

    <!-- Swap Button -->
    <button
      :disabled="!connected || !quote || loading"
      :class="[
        'w-full py-4 rounded-xl font-semibold text-lg transition-all',
        connected && quote && !loading
          ? 'bg-primary-600 hover:bg-primary-500 text-white'
          : 'bg-gray-700 text-gray-400 cursor-not-allowed'
      ]"
    >
      {{ !connected ? 'Connect Wallet' : loading ? 'Loading...' : 'Swap' }}
    </button>
  </div>
</template>

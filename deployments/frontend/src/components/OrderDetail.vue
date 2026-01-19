<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useOrders, type TrackedOrder } from '../composables/useOrders'
import { useFilledAmounts } from '../composables/useFilledAmounts'
import { useAssets } from '../composables/useAssets'

const props = defineProps<{
  orderId: string
  network: 'local' | 'devnet' | 'mainnet'
}>()

const emit = defineEmits<{
  (e: 'back'): void
}>()

const { orders, fetchOrders, getOrder } = useOrders()
const { clearFilledAmounts } = useFilledAmounts()
const { assets } = useAssets()

// Selected order from the list
const selectedOrder = ref<TrackedOrder | undefined>(undefined)
const detailLoading = ref(false)
const detailError = ref<string | null>(null)

// Polling state
const POLL_INTERVAL = 10000 // 10 seconds
let pollTimer: ReturnType<typeof setInterval> | null = null
const isPolling = ref(false)

// Copy feedback state
const copiedField = ref<string | null>(null)

// Check if order is fully filled
const isFullyFilled = computed(() => {
  if (!selectedOrder.value) return false
  const totalAmountOut = BigInt(selectedOrder.value.amount_out)
  const filledAmountOut = BigInt(selectedOrder.value.filled_amount)
  return filledAmountOut >= totalAmountOut
})

// Calculate fill percentage
const fillPercentage = computed(() => {
  if (!selectedOrder.value) return 0
  const totalAmountOut = BigInt(selectedOrder.value.amount_out)
  if (totalAmountOut === 0n) return 0
  const filledAmountOut = BigInt(selectedOrder.value.filled_amount)
  // Calculate percentage (multiply by 100 first to avoid precision loss)
  const percentage = Number((filledAmountOut * 100n) / totalAmountOut)
  return Math.min(percentage, 100)
})

// Check if we should show the fill progress
const shouldShowFillProgress = computed(() => {
  return selectedOrder.value !== undefined
})

function formatAmount(amount: string): string {
  const num = parseInt(amount) / 10**6
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

function getStatusColor(status: string): { bg: string; text: string; dot: string; glow: string } {
  const colors: Record<string, { bg: string; text: string; dot: string; glow: string }> = {
    'completed': { bg: 'bg-emerald-500/10', text: 'text-emerald-400', dot: 'bg-emerald-500', glow: 'shadow-[0_0_20px_rgba(16,185,129,0.3)]' },
    'created': { bg: 'bg-accent-500/10', text: 'text-accent-400', dot: 'bg-accent-500', glow: 'shadow-[0_0_20px_rgba(6,182,212,0.3)]' },
    'cancel_requested': { bg: 'bg-amber-500/10', text: 'text-amber-400', dot: 'bg-amber-500', glow: 'shadow-[0_0_20px_rgba(245,158,11,0.3)]' },
    'pending': { bg: 'bg-blue-500/10', text: 'text-blue-400', dot: 'bg-blue-500', glow: 'shadow-[0_0_20px_rgba(59,130,246,0.3)]' },
    'failed': { bg: 'bg-rose-500/10', text: 'text-rose-400', dot: 'bg-rose-500', glow: 'shadow-[0_0_20px_rgba(244,63,94,0.3)]' },
  }
  return colors[status.toLowerCase()] || { bg: 'bg-surface-700/50', text: 'text-surface-300', dot: 'bg-surface-400', glow: '' }
}

function formatTimestamp(timestamp: number): string {
  if (timestamp === 0) return 'N/A'
  return new Date(timestamp * 1000).toLocaleString()
}

function getChainName(chainId: number): string {
  if (chainId === undefined || chainId === null || chainId === 0) {
    return 'Unknown Chain'
  }
  const chains: Record<number, string> = {
    1: 'Ethereum',
    8453: 'Base',
    42161: 'Arbitrum',
    11155111: 'Sepolia',
    84532: 'Base Sepolia',
    1399811149: 'Solana',
    1399811150: 'Solana Devnet',
  }
  return chains[chainId] || `Chain ${chainId}`
}

function truncateAddress(address: string, startChars: number = 6, endChars: number = 4): string {
  if (!address || address.length < startChars + endChars + 3) return address
  return `${address.slice(0, startChars)}...${address.slice(-endChars)}`
}

function getChainColor(chainId: number): string {
  const colors: Record<number, string> = {
    1: '#627eea',
    8453: '#0052ff',
    42161: '#28a0f0',
    11155111: '#627eea',
    84532: '#0052ff',
    1399811149: '#9945ff',
    1399811150: '#9945ff',
  }
  return colors[chainId] || '#64748b'
}

function getChainGradient(chainId: number): string {
  const gradients: Record<number, string> = {
    1: 'from-[#627eea] to-[#4a5fc1]',
    8453: 'from-[#0052ff] to-[#003acc]',
    42161: 'from-[#28a0f0] to-[#1c7ac0]',
    11155111: 'from-[#627eea] to-[#4a5fc1]',
    84532: 'from-[#0052ff] to-[#003acc]',
    1399811149: 'from-[#9945ff] to-[#14f195]',
    1399811150: 'from-[#9945ff] to-[#14f195]',
  }
  return gradients[chainId] || 'from-slate-500 to-slate-600'
}

// Look up asset by token address and chain ID
function getAssetByAddress(tokenAddress: string, chainId: number) {
  return assets.value.find(
    (asset) => asset.address.toLowerCase() === tokenAddress.toLowerCase() && asset.chainId === chainId
  )
}

// Computed token info for input token
const tokenInInfo = computed(() => {
  if (!selectedOrder.value) return null
  return getAssetByAddress(selectedOrder.value.token_in, selectedOrder.value.origin_chain_id)
})

// Computed token info for output token
const tokenOutInfo = computed(() => {
  if (!selectedOrder.value) return null
  return getAssetByAddress(selectedOrder.value.token_out, selectedOrder.value.dest_chain_id)
})

async function copyToClipboard(text: string, fieldName: string) {
  try {
    await navigator.clipboard.writeText(text)
    copiedField.value = fieldName
    setTimeout(() => {
      copiedField.value = null
    }, 2000)
  } catch (err) {
    console.error('Failed to copy:', err)
  }
}

function goBack() {
  stopPolling()
  selectedOrder.value = undefined
  clearFilledAmounts()
  emit('back')
}

async function loadOrderDetails() {
  if (!props.orderId) return

  detailLoading.value = true
  detailError.value = null

  try {
    // Fetch all orders if not already loaded
    if (orders.value.length === 0) {
      await fetchOrders()
    }

    // Find the order by ID
    const order = getOrder(props.orderId)
    if (order) {
      selectedOrder.value = order
    } else {
      detailError.value = 'Order not found'
    }
  } catch (err) {
    detailError.value = err instanceof Error ? err.message : 'Failed to load order'
  } finally {
    detailLoading.value = false
  }
}

// Refresh order data from solver
async function refreshOrder() {
  if (!props.orderId) return

  await fetchOrders()
  const order = getOrder(props.orderId)
  if (order) {
    selectedOrder.value = order
  }
}

// Start polling for order updates
function startPolling() {
  if (pollTimer || isFullyFilled.value) return

  isPolling.value = true

  // Poll every POLL_INTERVAL
  pollTimer = setInterval(() => {
    if (isFullyFilled.value) {
      stopPolling()
      return
    }
    refreshOrder()
  }, POLL_INTERVAL)
}

// Stop polling
function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
  isPolling.value = false
}

// Start polling when order is loaded and not completed
watch(selectedOrder, (order) => {
  if (order && order.status !== 'completed' && order.status !== 'cancelled' && order.status !== 'rejected') {
    startPolling()
  } else {
    stopPolling()
  }
}, { immediate: true })

// Stop polling when fully filled
watch(isFullyFilled, (filled) => {
  if (filled) {
    stopPolling()
  }
})

onMounted(loadOrderDetails)

watch(() => props.orderId, () => {
  stopPolling()
  loadOrderDetails()
})

onUnmounted(() => {
  stopPolling()
})
</script>

<template>
  <div class="w-full max-w-5xl mx-auto">
    <!-- Loading State -->
    <div v-if="detailLoading" class="glass-card rounded-3xl p-12">
      <div class="flex flex-col items-center gap-4">
        <div class="relative">
          <div class="w-16 h-16 rounded-full border-2 border-accent-500/20 border-t-accent-500 animate-spin"></div>
          <div class="absolute inset-0 flex items-center justify-center">
            <div class="w-8 h-8 rounded-full bg-accent-500/10"></div>
          </div>
        </div>
        <span class="text-surface-400 text-sm">Loading order details...</span>
      </div>
    </div>

    <!-- Error State -->
    <div v-else-if="detailError" class="glass-card rounded-3xl p-8">
      <div class="flex flex-col items-center gap-4">
        <div class="w-16 h-16 rounded-2xl bg-rose-500/10 flex items-center justify-center">
          <svg class="w-8 h-8 text-rose-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="text-center">
          <p class="text-rose-300 font-medium">{{ detailError }}</p>
          <button @click="goBack" class="mt-4 text-sm text-surface-400 hover:text-white transition-colors">
            &larr; Back to Orders
          </button>
        </div>
      </div>
    </div>

    <!-- Order Details -->
    <div v-else-if="selectedOrder" class="space-y-4 animate-in">
      <!-- Header Card -->
      <div class="glass-card rounded-3xl p-6">
        <div class="flex items-start justify-between gap-4 flex-wrap">
          <!-- Back & Title -->
          <div class="flex items-center gap-4">
            <button
              @click="goBack"
              class="p-2.5 hover:bg-white/5 rounded-xl transition-all duration-200 group border border-white/5 hover:border-white/10"
            >
              <svg class="w-5 h-5 text-surface-400 group-hover:text-white transition-colors" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M15 19l-7-7 7-7" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </button>
            <div>
              <h2 class="text-xl font-semibold text-white">Order Details</h2>
              <p class="text-surface-500 text-sm mt-0.5 font-mono">{{ truncateAddress(selectedOrder.order_id, 8, 8) }}</p>
            </div>
          </div>

          <!-- Status Badge -->
          <div class="flex items-center gap-3">
            <div v-if="isPolling && !isFullyFilled" class="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-800/60 border border-white/5">
              <span class="w-1.5 h-1.5 bg-accent-500 rounded-full animate-pulse"></span>
              <span class="text-xs text-surface-400">Live</span>
            </div>
            <span
              :class="[
                'flex items-center gap-2 px-4 py-2 rounded-xl text-sm font-semibold uppercase tracking-wide',
                getStatusColor(selectedOrder.status).bg,
                getStatusColor(selectedOrder.status).text,
                getStatusColor(selectedOrder.status).glow
              ]"
            >
              <span :class="['w-2 h-2 rounded-full', getStatusColor(selectedOrder.status).dot, selectedOrder.status !== 'completed' && selectedOrder.status !== 'failed' ? 'animate-pulse' : '']"></span>
              {{ selectedOrder.status }}
            </span>
          </div>
        </div>
      </div>

      <!-- Main Content Grid -->
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <!-- Left Column: Swap Flow (spans 2 cols on lg) -->
        <div class="lg:col-span-2 space-y-4">
          <!-- Swap Flow Card -->
          <div class="glass-card rounded-3xl p-6 overflow-hidden relative">
            <!-- Background decoration -->
            <div class="absolute inset-0 overflow-hidden pointer-events-none">
              <div class="absolute -top-24 -right-24 w-48 h-48 rounded-full opacity-30" :style="{ background: `radial-gradient(circle, ${getChainColor(selectedOrder.origin_chain_id)}40 0%, transparent 70%)` }"></div>
              <div class="absolute -bottom-24 -left-24 w-48 h-48 rounded-full opacity-30" :style="{ background: `radial-gradient(circle, ${getChainColor(selectedOrder.dest_chain_id)}40 0%, transparent 70%)` }"></div>
            </div>

            <div class="relative">
              <div class="flex items-center gap-2 mb-6">
                <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <span class="text-sm font-medium text-surface-300">Swap Flow</span>
              </div>

              <div class="flex items-stretch gap-4">
                <!-- From -->
                <div class="flex-1 p-5 rounded-2xl bg-slate-900/60 border border-white/5 relative overflow-hidden">
                  <div class="absolute top-0 left-0 w-full h-1 bg-gradient-to-r" :class="getChainGradient(selectedOrder.origin_chain_id)"></div>
                  <div class="text-xs text-surface-500 uppercase tracking-wider mb-2">From</div>

                  <!-- Token with icon -->
                  <div class="flex items-center gap-3 mb-3">
                    <img
                      v-if="tokenInInfo?.icon"
                      :src="tokenInInfo.icon"
                      :alt="tokenInInfo.ticker"
                      class="w-10 h-10 rounded-full ring-2 ring-white/10"
                    />
                    <div v-else class="w-10 h-10 rounded-full bg-slate-700 ring-2 ring-white/10 flex items-center justify-center text-sm font-bold text-surface-300">
                      ?
                    </div>
                    <div>
                      <div class="text-2xl font-bold text-white">
                        {{ formatAmount(selectedOrder.amount_in) }}
                      </div>
                      <div class="text-sm font-medium text-surface-300">
                        {{ tokenInInfo?.ticker || 'Unknown' }}
                      </div>
                    </div>
                  </div>

                  <!-- Chain -->
                  <div class="flex items-center gap-2 mb-3">
                    <div class="w-4 h-4 rounded-full bg-gradient-to-br" :class="getChainGradient(selectedOrder.origin_chain_id)"></div>
                    <span class="text-xs text-surface-400">{{ getChainName(selectedOrder.origin_chain_id) }}</span>
                  </div>

                  <!-- Token address -->
                  <div
                    class="font-mono text-xs text-surface-500 truncate cursor-pointer hover:text-surface-300 transition-colors group flex items-center gap-1"
                    :title="selectedOrder.token_in"
                    @click="copyToClipboard(selectedOrder.token_in, 'token_in')"
                  >
                    <span>{{ truncateAddress(selectedOrder.token_in, 8, 6) }}</span>
                    <svg v-if="copiedField === 'token_in'" class="w-3 h-3 text-emerald-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    <svg v-else class="w-3 h-3 opacity-0 group-hover:opacity-100 transition-opacity" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                      <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                    </svg>
                  </div>
                </div>

                <!-- Arrow -->
                <div class="flex flex-col items-center justify-center px-2">
                  <div class="w-12 h-12 rounded-xl bg-slate-800/80 border border-white/10 flex items-center justify-center">
                    <svg class="w-6 h-6 text-accent-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  </div>
                </div>

                <!-- To -->
                <div class="flex-1 p-5 rounded-2xl bg-slate-900/60 border border-white/5 relative overflow-hidden">
                  <div class="absolute top-0 left-0 w-full h-1 bg-gradient-to-r" :class="getChainGradient(selectedOrder.dest_chain_id)"></div>
                  <div class="text-xs text-surface-500 uppercase tracking-wider mb-2">To</div>

                  <!-- Token with icon -->
                  <div class="flex items-center gap-3 mb-3">
                    <img
                      v-if="tokenOutInfo?.icon"
                      :src="tokenOutInfo.icon"
                      :alt="tokenOutInfo.ticker"
                      class="w-10 h-10 rounded-full ring-2 ring-white/10"
                    />
                    <div v-else class="w-10 h-10 rounded-full bg-slate-700 ring-2 ring-white/10 flex items-center justify-center text-sm font-bold text-surface-300">
                      ?
                    </div>
                    <div>
                      <div class="text-2xl font-bold text-white">
                        {{ formatAmount(selectedOrder.amount_out) }}
                      </div>
                      <div class="text-sm font-medium text-surface-300">
                        {{ tokenOutInfo?.ticker || 'Unknown' }}
                      </div>
                    </div>
                  </div>

                  <!-- Chain -->
                  <div class="flex items-center gap-2 mb-3">
                    <div class="w-4 h-4 rounded-full bg-gradient-to-br" :class="getChainGradient(selectedOrder.dest_chain_id)"></div>
                    <span class="text-xs text-surface-400">{{ getChainName(selectedOrder.dest_chain_id) }}</span>
                  </div>

                  <!-- Token address -->
                  <div
                    class="font-mono text-xs text-surface-500 truncate cursor-pointer hover:text-surface-300 transition-colors group flex items-center gap-1"
                    :title="selectedOrder.token_out"
                    @click="copyToClipboard(selectedOrder.token_out, 'token_out')"
                  >
                    <span>{{ truncateAddress(selectedOrder.token_out, 8, 6) }}</span>
                    <svg v-if="copiedField === 'token_out'" class="w-3 h-3 text-emerald-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    <svg v-else class="w-3 h-3 opacity-0 group-hover:opacity-100 transition-opacity" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                      <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                    </svg>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Fill Progress Card -->
          <div v-if="shouldShowFillProgress" class="glass-card rounded-3xl p-6">
            <div class="flex items-center justify-between mb-4">
              <div class="flex items-center gap-2">
                <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <span class="text-sm font-medium text-surface-300">Fill Progress</span>
              </div>
              <span
                class="text-2xl font-bold tabular-nums"
                :class="isFullyFilled ? 'text-emerald-400' : 'text-white'"
              >
                {{ fillPercentage.toFixed(1) }}%
              </span>
            </div>

            <!-- Progress Bar -->
            <div class="relative h-4 bg-slate-800/80 rounded-full overflow-hidden mb-4">
              <div
                class="absolute inset-y-0 left-0 rounded-full transition-all duration-700 ease-out"
                :class="isFullyFilled ? 'bg-gradient-to-r from-emerald-500 to-emerald-400' : 'bg-gradient-to-r from-accent-600 via-accent-500 to-accent-400'"
                :style="{ width: `${fillPercentage}%` }"
              ></div>
              <!-- Animated shine effect when not fully filled -->
              <div
                v-if="!isFullyFilled && fillPercentage > 0"
                class="absolute inset-y-0 left-0 bg-gradient-to-r from-transparent via-white/30 to-transparent animate-shimmer"
                :style="{ width: `${fillPercentage}%` }"
              ></div>
            </div>

            <!-- Fill Details -->
            <div class="flex items-center justify-between">
              <div class="text-sm text-surface-400">
                <span class="text-white font-medium">{{ formatAmount(selectedOrder.filled_amount) }}</span>
                <span> / {{ formatAmount(selectedOrder.amount_out) }}</span>
              </div>
              <div v-if="isFullyFilled" class="flex items-center gap-1.5 text-emerald-400 text-sm font-medium">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                Complete
              </div>
            </div>
          </div>

          <!-- Transaction History -->
          <div v-if="selectedOrder.transaction_history && selectedOrder.transaction_history.length > 0" class="glass-card rounded-3xl p-6">
            <div class="flex items-center gap-2 mb-4">
              <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <span class="text-sm font-medium text-surface-300">Transaction History</span>
              <span class="ml-auto text-xs text-surface-500 bg-slate-800/60 px-2 py-0.5 rounded-full">
                {{ selectedOrder.transaction_history.length }} {{ selectedOrder.transaction_history.length === 1 ? 'tx' : 'txs' }}
              </span>
            </div>

            <div class="space-y-2">
              <div
                v-for="(tx, index) in selectedOrder.transaction_history"
                :key="index"
                class="flex items-center justify-between p-3 bg-slate-900/50 rounded-xl border border-white/5 cursor-pointer hover:bg-slate-800/60 hover:border-white/10 transition-all duration-200 group"
                @click="copyToClipboard(tx.transaction_hash, `tx_${index}`)"
              >
                <div class="flex items-center gap-3">
                  <div class="w-8 h-8 rounded-lg bg-accent-500/10 flex items-center justify-center flex-shrink-0 group-hover:bg-accent-500/20 transition-colors">
                    <svg class="w-4 h-4 text-accent-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M13 10V3L4 14h7v7l9-11h-7z" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  </div>
                  <div>
                    <div class="text-sm font-medium text-surface-200">{{ tx.event }}</div>
                    <div class="text-xs text-surface-500 font-mono mt-0.5">{{ truncateAddress(tx.transaction_hash, 10, 8) }}</div>
                  </div>
                </div>
                <div class="flex items-center gap-2">
                  <svg v-if="copiedField === `tx_${index}`" class="w-4 h-4 text-emerald-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  <svg v-else class="w-4 h-4 text-surface-500 opacity-0 group-hover:opacity-100 transition-opacity" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                  </svg>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Right Column: Details -->
        <div class="space-y-4">
          <!-- Order Info Card -->
          <div class="glass-card rounded-3xl p-6">
            <div class="flex items-center gap-2 mb-4">
              <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <span class="text-sm font-medium text-surface-300">Order Info</span>
            </div>

            <div class="space-y-4">
              <!-- Order ID -->
              <div>
                <div class="text-xs text-surface-500 mb-1">Order ID</div>
                <div
                  class="font-mono text-xs text-surface-200 bg-slate-900/60 rounded-lg p-2.5 break-all cursor-pointer hover:bg-slate-800/60 transition-colors group flex items-center justify-between gap-2"
                  @click="copyToClipboard(selectedOrder.order_id, 'order_id')"
                >
                  <span class="break-all">{{ selectedOrder.order_id }}</span>
                  <svg v-if="copiedField === 'order_id'" class="w-4 h-4 text-emerald-400 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  <svg v-else class="w-4 h-4 text-surface-500 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                  </svg>
                </div>
              </div>

              <!-- Grid for small details -->
              <div class="grid grid-cols-2 gap-3">
                <div class="bg-slate-900/40 rounded-lg p-3">
                  <div class="text-xs text-surface-500 mb-0.5">Version</div>
                  <div class="text-sm text-white font-medium font-mono">{{ selectedOrder.version }}</div>
                </div>
                <div class="bg-slate-900/40 rounded-lg p-3">
                  <div class="text-xs text-surface-500 mb-0.5">Nonce</div>
                  <div class="text-sm text-white font-medium font-mono">{{ selectedOrder.nonce }}</div>
                </div>
              </div>

              <!-- Fill Deadline -->
              <div class="bg-slate-900/40 rounded-lg p-3">
                <div class="text-xs text-surface-500 mb-0.5">Fill Deadline</div>
                <div class="text-sm text-white font-medium">{{ formatTimestamp(selectedOrder.fill_deadline) }}</div>
              </div>
            </div>
          </div>

          <!-- Addresses Card -->
          <div class="glass-card rounded-3xl p-6">
            <div class="flex items-center gap-2 mb-4">
              <svg class="w-5 h-5 text-surface-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <span class="text-sm font-medium text-surface-300">Addresses</span>
            </div>

            <div class="space-y-3">
              <!-- Sender -->
              <div>
                <div class="flex items-center gap-1.5 text-xs text-surface-500 mb-1">
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  Sender
                </div>
                <div
                  class="font-mono text-xs text-surface-200 bg-slate-900/60 rounded-lg p-2.5 break-all cursor-pointer hover:bg-slate-800/60 transition-colors group flex items-center justify-between gap-2"
                  @click="copyToClipboard(selectedOrder.sender, 'sender')"
                >
                  <span class="break-all">{{ selectedOrder.sender }}</span>
                  <svg v-if="copiedField === 'sender'" class="w-4 h-4 text-emerald-400 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  <svg v-else class="w-4 h-4 text-surface-500 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                  </svg>
                </div>
              </div>

              <!-- Recipient -->
              <div>
                <div class="flex items-center gap-1.5 text-xs text-surface-500 mb-1">
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  Recipient
                </div>
                <div
                  class="font-mono text-xs text-surface-200 bg-slate-900/60 rounded-lg p-2.5 break-all cursor-pointer hover:bg-slate-800/60 transition-colors group flex items-center justify-between gap-2"
                  @click="copyToClipboard(selectedOrder.recipient, 'recipient')"
                >
                  <span class="break-all">{{ selectedOrder.recipient }}</span>
                  <svg v-if="copiedField === 'recipient'" class="w-4 h-4 text-emerald-400 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  <svg v-else class="w-4 h-4 text-surface-500 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                  </svg>
                </div>
              </div>

              <!-- Solver -->
              <div>
                <div class="flex items-center gap-1.5 text-xs text-surface-500 mb-1">
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  Solver
                </div>
                <div
                  class="font-mono text-xs text-accent-400 bg-accent-500/5 border border-accent-500/10 rounded-lg p-2.5 break-all cursor-pointer hover:bg-accent-500/10 transition-colors group flex items-center justify-between gap-2"
                  @click="copyToClipboard(selectedOrder.solver, 'solver')"
                >
                  <span class="break-all">{{ selectedOrder.solver }}</span>
                  <svg v-if="copiedField === 'solver'" class="w-4 h-4 text-emerald-400 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                  <svg v-else class="w-4 h-4 text-accent-400 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
                  </svg>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Not Found State -->
    <div v-else class="glass-card rounded-3xl p-12">
      <div class="flex flex-col items-center gap-4">
        <div class="w-20 h-20 rounded-2xl bg-slate-800/50 flex items-center justify-center">
          <svg class="w-10 h-10 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="text-center">
          <p class="text-surface-300 font-medium text-lg">Order not found</p>
          <p class="text-surface-500 text-sm mt-1">The order may have been deleted or doesn't exist</p>
          <button @click="goBack" class="mt-4 text-sm text-accent-400 hover:text-accent-300 transition-colors">
            &larr; Back to Orders
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

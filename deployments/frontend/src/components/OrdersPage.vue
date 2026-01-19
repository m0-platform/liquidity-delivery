<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useOrders, type TrackedOrder } from '../composables/useOrders'
import { useAssets } from '../composables/useAssets'
import type { NetworkType } from '../config/network'

const props = defineProps<{
  walletAddress: string | null
  network: NetworkType
}>()

const emit = defineEmits<{
  (e: 'select-order', orderId: string): void
}>()

const networkRef = computed(() => props.network)
const { orders, loading, error, fetchOrders, getOrdersBySender } = useOrders(networkRef)
const { assets } = useAssets(networkRef)

const showMyOrdersOnly = ref(false)

async function loadOrders() {
  await fetchOrders()
}

// Computed filtered orders for display
const displayOrders = computed(() => {
  if (showMyOrdersOnly.value && props.walletAddress) {
    return getOrdersBySender(props.walletAddress)
  }
  return orders.value
})

function selectOrder(order: TrackedOrder) {
  emit('select-order', order.order_id)
}

function truncateAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 4)}…${address.slice(-8)}`
}

function formatAmount(amount: string): string {
  const num = parseInt(amount) / 10**6
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

function getTokenTicker(address: string): string {
  const asset = assets.value.find(a => a.address.toLowerCase() === address.toLowerCase())
  return asset?.symbol || truncateAddress(address)
}

function getFillPercentage(order: TrackedOrder): number {
  const filled = parseInt(order.filled_amount) || 0
  const total = parseInt(order.amount_out) || 0
  if (total === 0) return 0
  return Math.min(100, (filled / total) * 100)
}

function getFillStatusColor(percentage: number): string {
  if (percentage >= 100) return '#22c55e' // green
  if (percentage > 0) return '#f59e0b' // amber
  return '#64748b' // slate
}

function getChainName(chainId: number): string {
  if (chainId === undefined || chainId === null || chainId === 0) {
    return 'Unknown'
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

// Load orders on mount
onMounted(loadOrders)
</script>

<template>
  <div class="glass-card rounded-3xl p-6">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <h2 class="text-xl font-semibold text-white">Orders</h2>
      <button
        @click="loadOrders"
        :disabled="loading"
        class="btn-secondary flex items-center gap-2 text-sm"
      >
        <svg
          :class="['w-4 h-4 transition-transform', loading && 'animate-spin']"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Refresh
      </button>
    </div>

    <!-- Filter -->
    <div class="mb-6">
      <label class="flex items-center gap-3 cursor-pointer group">
        <div class="relative">
          <input
            type="checkbox"
            v-model="showMyOrdersOnly"
            :disabled="!walletAddress"
            class="sr-only peer"
          />
          <div class="w-10 h-6 bg-slate-700 rounded-full peer peer-checked:bg-accent-600 transition-colors"></div>
          <div class="absolute left-1 top-1 w-4 h-4 bg-white rounded-full transition-transform peer-checked:translate-x-4"></div>
        </div>
        <span :class="['text-sm transition-colors', walletAddress ? 'text-surface-300 group-hover:text-white' : 'text-surface-500']">
          Show my orders only
        </span>
      </label>
      <p v-if="!walletAddress && showMyOrdersOnly" class="text-xs text-surface-500 mt-2 ml-13">
        Connect wallet to filter by your orders
      </p>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="py-12">
      <div class="flex flex-col items-center gap-4">
        <div class="loading-spinner w-8 h-8"></div>
        <span class="text-surface-400 text-sm">Loading orders...</span>
      </div>
    </div>

    <!-- Error State -->
    <div v-else-if="error" class="bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 flex items-start gap-3">
      <svg class="w-5 h-5 flex-shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span class="text-sm">{{ error }}</span>
    </div>

    <!-- Empty State -->
    <div v-else-if="displayOrders.length === 0" class="py-12">
      <div class="flex flex-col items-center gap-4">
        <div class="w-16 h-16 rounded-2xl bg-slate-800/50 flex items-center justify-center">
          <svg class="w-8 h-8 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="text-center">
          <p class="text-surface-300 font-medium">No orders found</p>
          <p class="text-surface-500 text-sm mt-1">Orders will appear here once created</p>
        </div>
      </div>
    </div>

    <!-- Orders List -->
    <div v-else class="space-y-3 max-h-[60vh] overflow-y-auto pr-2 scrollbar-thin scrollbar-thumb-surface-600 scrollbar-track-transparent">
      <TransitionGroup
        enter-active-class="transition-all duration-300 ease-out"
        enter-from-class="opacity-0 translate-y-2"
        enter-to-class="opacity-100 translate-y-0"
        leave-active-class="transition-all duration-200 ease-in"
        leave-from-class="opacity-100 translate-y-0"
        leave-to-class="opacity-0 translate-y-2"
      >
        <div
          v-for="(order, index) in displayOrders"
          :key="order.order_id"
          @click="selectOrder(order)"
          class="group bg-slate-925/60 rounded-xl p-4 cursor-pointer border border-white/5 hover:border-accent-500/30 hover:bg-slate-900/80 transition-all duration-200 overflow-hidden"
          :style="{ animationDelay: `${index * 50}ms` }"
        >
          <!-- Order Header -->
          <div class="flex items-center justify-between mb-3">
            <span class="text-xs text-surface-500 font-mono">
              {{ truncateAddress(order.order_id) }}
            </span>
            <div class="flex items-center gap-2">
              <span
                class="flex items-center gap-1.5 text-xs px-2 py-1 rounded-lg border border-white/10"
                :style="{ backgroundColor: `${getChainColor(order.origin_chain_id)}15` }"
              >
                <span class="w-1.5 h-1.5 rounded-full" :style="{ backgroundColor: getChainColor(order.origin_chain_id) }"></span>
                <span :style="{ color: getChainColor(order.origin_chain_id) }">{{ getChainName(order.origin_chain_id) }}</span>
              </span>
              <svg class="w-3 h-3 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              <span
                class="flex items-center gap-1.5 text-xs px-2 py-1 rounded-lg border border-white/10"
                :style="{ backgroundColor: `${getChainColor(order.dest_chain_id)}15` }"
              >
                <span class="w-1.5 h-1.5 rounded-full" :style="{ backgroundColor: getChainColor(order.dest_chain_id) }"></span>
                <span :style="{ color: getChainColor(order.dest_chain_id) }">{{ getChainName(order.dest_chain_id) }}</span>
              </span>
            </div>
          </div>

          <!-- Amount Row -->
          <div class="flex items-center justify-between gap-3 overflow-hidden">
            <div class="flex items-center gap-2 min-w-0 flex-1">
              <div class="text-lg font-semibold text-white flex-shrink-0">{{ formatAmount(order.amount_in) }}</div>
              <span class="text-surface-400 text-sm font-medium">{{ getTokenTicker(order.token_in) }}</span>
            </div>
            <svg class="w-5 h-5 text-accent-500 group-hover:translate-x-1 transition-transform flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <div class="flex items-center gap-2 min-w-0 flex-1 justify-end">
              <div class="text-lg font-semibold text-white flex-shrink-0">{{ formatAmount(order.amount_out) }}</div>
              <span class="text-surface-400 text-sm font-medium">{{ getTokenTicker(order.token_out) }}</span>
            </div>
          </div>

          <!-- Fill Progress -->
          <div class="mt-3 pt-3 border-t border-white/5">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs text-surface-500">Fill Progress</span>
              <span
                class="text-xs font-medium"
                :style="{ color: getFillStatusColor(getFillPercentage(order)) }"
              >
                {{ getFillPercentage(order).toFixed(1) }}%
              </span>
            </div>
            <div class="h-1.5 bg-slate-700/50 rounded-full overflow-hidden">
              <div
                class="h-full rounded-full transition-all duration-500 ease-out"
                :style="{
                  width: `${getFillPercentage(order)}%`,
                  backgroundColor: getFillStatusColor(getFillPercentage(order))
                }"
              ></div>
            </div>
            <div class="flex items-center justify-between mt-1.5">
              <span class="text-[10px] text-surface-500">
                {{ formatAmount(order.filled_amount) }} filled
              </span>
              <span class="text-[10px] text-surface-500">
                {{ formatAmount(order.amount_out) }} total
              </span>
            </div>
          </div>

        </div>
      </TransitionGroup>
    </div>

    <!-- Order Count -->
    <div v-if="displayOrders.length > 0" class="mt-6 pt-4 border-t border-white/5 text-center">
      <span class="text-sm text-surface-500">
        {{ displayOrders.length }} order{{ displayOrders.length === 1 ? '' : 's' }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { watch, onMounted } from 'vue'
import { useOrders } from '../composables/useOrders'

const props = defineProps<{
  orderId: string
}>()

const emit = defineEmits<{
  (e: 'back'): void
}>()

const { selectedOrder, detailLoading, detailError, fetchOrderDetails, clearSelectedOrder } = useOrders()

function formatAmount(amount: string): string {
  const num = parseInt(amount) / 10**6
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

function getStatusColor(status: string): { bg: string; text: string; dot: string } {
  const colors: Record<string, { bg: string; text: string; dot: string }> = {
    'completed': { bg: 'bg-emerald-500/10', text: 'text-emerald-400', dot: 'bg-emerald-500' },
    'created': { bg: 'bg-accent-500/10', text: 'text-accent-400', dot: 'bg-accent-500' },
    'cancel_requested': { bg: 'bg-amber-500/10', text: 'text-amber-400', dot: 'bg-amber-500' },
    'pending': { bg: 'bg-blue-500/10', text: 'text-blue-400', dot: 'bg-blue-500' },
    'failed': { bg: 'bg-rose-500/10', text: 'text-rose-400', dot: 'bg-rose-500' },
  }
  return colors[status.toLowerCase()] || { bg: 'bg-surface-700/50', text: 'text-surface-300', dot: 'bg-surface-400' }
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
    1: 'Ethereum Mainnet',
    8453: 'Base',
    42161: 'Arbitrum One',
    11155111: 'Sepolia Testnet',
    84532: 'Base Sepolia',
    31337: 'Anvil (Local)',
    1399811149: 'Solana',
    1399811150: 'Solana Devnet',
  }
  return chains[chainId] || `Chain ${chainId}`
}

function truncateAddress(address: string): string {
  if (!address || address.length < 16) return address
  return `${address.slice(0, 4)}…${address.slice(-12)}`
}

function getChainColor(chainId: number): string {
  const colors: Record<number, string> = {
    1: '#627eea',
    8453: '#0052ff',
    42161: '#28a0f0',
    11155111: '#627eea',
    84532: '#0052ff',
    31337: '#10b981',
    1399811149: '#9945ff',
    1399811150: '#9945ff',
  }
  return colors[chainId] || '#64748b'
}

function goBack() {
  clearSelectedOrder()
  emit('back')
}

async function loadOrderDetails() {
  if (props.orderId) {
    await fetchOrderDetails(props.orderId)
  }
}

onMounted(loadOrderDetails)

watch(() => props.orderId, loadOrderDetails)
</script>

<template>
  <div class="glass-card rounded-3xl p-6">
    <!-- Header -->
    <div class="flex items-center gap-4 mb-6">
      <button
        @click="goBack"
        class="p-2 hover:bg-white/5 rounded-xl transition-colors group"
      >
        <svg class="w-5 h-5 text-surface-400 group-hover:text-white transition-colors" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M15 19l-7-7 7-7" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
      <h2 class="text-xl font-semibold text-white">Order Details</h2>
    </div>

    <!-- Loading State -->
    <div v-if="detailLoading" class="py-12">
      <div class="flex flex-col items-center gap-4">
        <div class="loading-spinner w-8 h-8"></div>
        <span class="text-surface-400 text-sm">Loading order details...</span>
      </div>
    </div>

    <!-- Error State -->
    <div v-else-if="detailError" class="bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 flex items-start gap-3">
      <svg class="w-5 h-5 flex-shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span class="text-sm">{{ detailError }}</span>
    </div>

    <!-- Order Details -->
    <div v-else-if="selectedOrder" class="space-y-4 animate-in">
      <!-- Status Badge -->
      <div class="flex items-center justify-between p-4 bg-slate-925/60 rounded-xl border border-white/5">
        <span class="text-sm text-surface-400">Status</span>
        <span
          :class="[
            'flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium',
            getStatusColor(selectedOrder.status).bg,
            getStatusColor(selectedOrder.status).text
          ]"
        >
          <span :class="['w-2 h-2 rounded-full animate-pulse', getStatusColor(selectedOrder.status).dot]"></span>
          {{ selectedOrder.status }}
        </span>
      </div>

      <!-- Order ID -->
      <div class="p-4 bg-slate-925/60 rounded-xl border border-white/5">
        <div class="text-xs text-surface-500 mb-2 flex items-center gap-2">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Order ID
        </div>
        <div class="font-mono text-sm text-surface-200 break-all select-all">{{ selectedOrder.order_id }}</div>
      </div>

      <!-- Amount Details -->
      <div class="p-4 bg-slate-925/60 rounded-xl border border-white/5 overflow-hidden">
        <div class="flex items-center justify-between mb-6 gap-4">
          <!-- Input -->
          <div class="flex-1 min-w-0">
            <div class="text-xs text-surface-500 mb-1">Input</div>
            <div class="text-2xl font-semibold text-white truncate">
              {{ formatAmount(selectedOrder.amount_in) }}
            </div>
            <span class="text-surface-400 text-sm font-mono truncate block" :title="selectedOrder.token_in">
              {{ truncateAddress(selectedOrder.token_in) }}
            </span>
          </div>

          <!-- Arrow -->
          <div class="flex flex-col items-center gap-1 flex-shrink-0">
            <svg class="w-8 h-8 text-accent-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>

          <!-- Output -->
          <div class="text-right flex-1 min-w-0">
            <div class="text-xs text-surface-500 mb-1">Output</div>
            <div class="text-2xl font-semibold text-white truncate">
              {{ formatAmount(selectedOrder.amount_out) }}
            </div>
            <span class="text-surface-400 text-sm font-mono truncate block" :title="selectedOrder.token_out">
              {{ truncateAddress(selectedOrder.token_out) }}
            </span>
          </div>
        </div>

        <!-- Chain Route -->
        <div class="pt-4 border-t border-white/5">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <div
                class="w-3 h-3 rounded-full"
                :style="{ backgroundColor: getChainColor(selectedOrder.origin_chain_id) }"
              ></div>
              <span class="text-sm text-surface-300">{{ getChainName(selectedOrder.origin_chain_id) }}</span>
            </div>
            <svg class="w-4 h-4 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M14 5l7 7m0 0l-7 7m7-7H3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <div class="flex items-center gap-2">
              <span class="text-sm text-surface-300">{{ getChainName(selectedOrder.dest_chain_id) }}</span>
              <div
                class="w-3 h-3 rounded-full"
                :style="{ backgroundColor: getChainColor(selectedOrder.dest_chain_id) }"
              ></div>
            </div>
          </div>
        </div>
      </div>

      <!-- Addresses -->
      <div class="p-4 bg-slate-925/60 rounded-xl border border-white/5 space-y-4">
        <div>
          <div class="text-xs text-surface-500 mb-2 flex items-center gap-2">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            Sender
          </div>
          <div class="font-mono text-sm text-surface-200 break-all select-all">{{ selectedOrder.sender }}</div>
        </div>

        <div>
          <div class="text-xs text-surface-500 mb-2 flex items-center gap-2">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M15 11a3 3 0 11-6 0 3 3 0 016 0z" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            Recipient
          </div>
          <div class="font-mono text-sm text-surface-200 break-all select-all">{{ selectedOrder.recipient }}</div>
        </div>

        <div>
          <div class="text-xs text-surface-500 mb-2 flex items-center gap-2">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            Solver
          </div>
          <div class="font-mono text-sm text-accent-400 break-all select-all">{{ selectedOrder.solver }}</div>
        </div>
      </div>

      <!-- Additional Info -->
      <div class="p-4 bg-slate-925/60 rounded-xl border border-white/5">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <div class="text-xs text-surface-500 mb-1">Version</div>
            <div class="text-sm text-surface-200 font-mono">{{ selectedOrder.version }}</div>
          </div>
          <div>
            <div class="text-xs text-surface-500 mb-1">Nonce</div>
            <div class="text-sm text-surface-200 font-mono">{{ selectedOrder.nonce }}</div>
          </div>
          <div>
            <div class="text-xs text-surface-500 mb-1">Fill Deadline</div>
            <div class="text-sm text-surface-200">{{ formatTimestamp(selectedOrder.fill_deadline) }}</div>
          </div>
          <div>
            <div class="text-xs text-surface-500 mb-1">Cancel Requested</div>
            <div class="text-sm text-surface-200">{{ formatTimestamp(selectedOrder.cancel_requested_at) }}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Not Found State -->
    <div v-else class="py-12">
      <div class="flex flex-col items-center gap-4">
        <div class="w-16 h-16 rounded-2xl bg-slate-800/50 flex items-center justify-center">
          <svg class="w-8 h-8 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="text-center">
          <p class="text-surface-300 font-medium">Order not found</p>
          <p class="text-surface-500 text-sm mt-1">The order may have been deleted or doesn't exist</p>
        </div>
      </div>
    </div>
  </div>
</template>

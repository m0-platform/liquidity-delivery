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
  const num = parseFloat(amount)
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

function getStatusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'completed':
      return 'bg-green-900/50 text-green-300'
    case 'created':
      return 'bg-blue-900/50 text-blue-300'
    case 'cancel_requested':
      return 'bg-yellow-900/50 text-yellow-300'
    default:
      return 'bg-gray-700 text-gray-300'
  }
}

function formatTimestamp(timestamp: number): string {
  if (timestamp === 0) return 'N/A'
  return new Date(timestamp * 1000).toLocaleString()
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
  <div class="bg-gray-800 rounded-2xl p-6 shadow-xl border border-gray-700">
    <!-- Header -->
    <div class="flex items-center gap-3 mb-6">
      <button
        @click="goBack"
        class="p-2 hover:bg-gray-700 rounded-lg transition-colors"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
        </svg>
      </button>
      <h2 class="text-lg font-semibold">Order Details</h2>
    </div>

    <!-- Loading State -->
    <div v-if="detailLoading" class="text-center text-gray-400 py-8">
      Loading order details...
    </div>

    <!-- Error State -->
    <div v-else-if="detailError" class="bg-red-900/50 text-red-300 rounded-xl p-4 text-sm">
      {{ detailError }}
    </div>

    <!-- Order Details -->
    <div v-else-if="selectedOrder" class="space-y-4">
      <!-- Status Badge -->
      <div class="flex items-center justify-between">
        <span class="text-sm text-gray-400">Status</span>
        <span :class="['px-3 py-1 rounded-full text-sm font-medium', getStatusColor(selectedOrder.status)]">
          {{ selectedOrder.status }}
        </span>
      </div>

      <!-- Order ID -->
      <div class="bg-gray-900 rounded-xl p-4">
        <div class="text-xs text-gray-500 mb-1">Order ID</div>
        <div class="font-mono text-sm break-all">{{ selectedOrder.order_id }}</div>
      </div>

      <!-- Amount Details -->
      <div class="bg-gray-900 rounded-xl p-4">
        <div class="flex items-center justify-between mb-4">
          <div>
            <div class="text-xs text-gray-500 mb-1">Input</div>
            <div class="text-lg font-medium">
              {{ formatAmount(selectedOrder.amount_in) }}
              <span class="text-gray-400 text-sm ml-1">{{ selectedOrder.token_in }}</span>
            </div>
          </div>
          <svg class="w-6 h-6 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
          </svg>
          <div class="text-right">
            <div class="text-xs text-gray-500 mb-1">Output</div>
            <div class="text-lg font-medium">
              {{ formatAmount(selectedOrder.amount_out) }}
              <span class="text-gray-400 text-sm ml-1">{{ selectedOrder.token_out }}</span>
            </div>
          </div>
        </div>

        <div class="border-t border-gray-700 pt-3 mt-3">
          <div class="text-xs text-gray-500 mb-1">Destination Chain</div>
          <div class="text-sm">Chain ID: {{ selectedOrder.dest_chain_id }}</div>
        </div>
      </div>

      <!-- Addresses -->
      <div class="bg-gray-900 rounded-xl p-4 space-y-3">
        <div>
          <div class="text-xs text-gray-500 mb-1">Sender</div>
          <div class="font-mono text-sm break-all">{{ selectedOrder.sender }}</div>
        </div>

        <div>
          <div class="text-xs text-gray-500 mb-1">Recipient</div>
          <div class="font-mono text-sm break-all">{{ selectedOrder.recipient }}</div>
        </div>

        <div>
          <div class="text-xs text-gray-500 mb-1">Solver</div>
          <div class="font-mono text-sm break-all">{{ selectedOrder.solver }}</div>
        </div>
      </div>

      <!-- Additional Info -->
      <div class="bg-gray-900 rounded-xl p-4">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <div class="text-xs text-gray-500 mb-1">Version</div>
            <div class="text-sm">{{ selectedOrder.version }}</div>
          </div>
          <div>
            <div class="text-xs text-gray-500 mb-1">Nonce</div>
            <div class="text-sm">{{ selectedOrder.nonce }}</div>
          </div>
          <div>
            <div class="text-xs text-gray-500 mb-1">Fill Deadline</div>
            <div class="text-sm">{{ formatTimestamp(selectedOrder.fill_deadline) }}</div>
          </div>
          <div>
            <div class="text-xs text-gray-500 mb-1">Cancel Requested</div>
            <div class="text-sm">{{ formatTimestamp(selectedOrder.cancel_requested_at) }}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Not Found State -->
    <div v-else class="text-center text-gray-400 py-8">
      Order not found
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useOrders, type TrackedOrder } from '../composables/useOrders'

const props = defineProps<{
  walletAddress: string | null
}>()

const emit = defineEmits<{
  (e: 'select-order', orderId: string): void
}>()

const { orders, loading, error, fetchOrders } = useOrders()

const showMyOrdersOnly = ref(false)

async function loadOrders() {
  const sender = showMyOrdersOnly.value && props.walletAddress
    ? props.walletAddress
    : undefined
  await fetchOrders(sender)
}

function selectOrder(order: TrackedOrder) {
  emit('select-order', order.order_id)
}

function truncateAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 6)}...${address.slice(-4)}`
}

function formatAmount(amount: string): string {
  const num = parseFloat(amount)
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 6 })
}

// Load orders on mount and when filter changes
onMounted(loadOrders)

watch([showMyOrdersOnly, () => props.walletAddress], loadOrders)
</script>

<template>
  <div class="bg-gray-800 rounded-2xl p-6 shadow-xl border border-gray-700">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">Orders</h2>
      <button
        @click="loadOrders"
        :disabled="loading"
        class="text-sm text-primary-400 hover:text-primary-300 disabled:text-gray-500"
      >
        Refresh
      </button>
    </div>

    <!-- Filter -->
    <div class="mb-4">
      <label class="flex items-center gap-2 text-sm text-gray-400 cursor-pointer">
        <input
          type="checkbox"
          v-model="showMyOrdersOnly"
          :disabled="!walletAddress"
          class="rounded bg-gray-700 border-gray-600 text-primary-500 focus:ring-primary-500"
        />
        <span :class="{ 'opacity-50': !walletAddress }">
          Show my orders only
        </span>
      </label>
      <p v-if="!walletAddress && showMyOrdersOnly" class="text-xs text-gray-500 mt-1">
        Connect wallet to filter by your orders
      </p>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="text-center text-gray-400 py-8">
      Loading orders...
    </div>

    <!-- Error State -->
    <div v-else-if="error" class="bg-red-900/50 text-red-300 rounded-xl p-4 text-sm">
      {{ error }}
    </div>

    <!-- Empty State -->
    <div v-else-if="orders.length === 0" class="text-center text-gray-400 py-8">
      No orders found
    </div>

    <!-- Orders List -->
    <div v-else class="space-y-2">
      <div
        v-for="order in orders"
        :key="order.order_id"
        @click="selectOrder(order)"
        class="bg-gray-900 rounded-xl p-4 cursor-pointer hover:bg-gray-850 transition-colors border border-transparent hover:border-gray-700"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-xs text-gray-500 font-mono">
            {{ truncateAddress(order.order_id) }}
          </span>
          <span class="text-xs px-2 py-0.5 rounded bg-gray-700 text-gray-300">
            Chain {{ order.origin_chain_id }} → {{ order.dest_chain_id }}
          </span>
        </div>

        <div class="flex items-center justify-between">
          <div class="text-sm">
            <span class="text-white font-medium">{{ formatAmount(order.amount_in) }}</span>
            <span class="text-gray-400 ml-1">{{ order.token_in }}</span>
          </div>
          <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
          </svg>
          <div class="text-sm text-right">
            <span class="text-white font-medium">{{ formatAmount(order.amount_out) }}</span>
            <span class="text-gray-400 ml-1">{{ order.token_out }}</span>
          </div>
        </div>

        <div class="mt-2 text-xs text-gray-500">
          From: {{ truncateAddress(order.sender) }}
        </div>
      </div>
    </div>

    <!-- Order Count -->
    <div v-if="orders.length > 0" class="mt-4 text-center text-sm text-gray-500">
      {{ orders.length }} order{{ orders.length === 1 ? '' : 's' }}
    </div>
  </div>
</template>

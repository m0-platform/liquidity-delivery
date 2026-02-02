<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useOrders, type TrackedOrder } from '../composables/useOrders'
import { useAssets } from '../composables/useAssets'
import type { NetworkType } from '../config/network'

const props = defineProps<{
  evmAddress: string | null
  svmAddress: string | null
  network: NetworkType
}>()

const emit = defineEmits<{
  (e: 'select-order', orderId: string): void
}>()

const networkRef = computed(() => props.network)
const { orders, loading, error, fetchOrders, getOrdersBySender } = useOrders(networkRef)
const { assets } = useAssets(networkRef)

const showMyOrdersOnly = ref(false)
const viewMode = ref<'table' | 'grid'>('table')
const sortBy = ref<'date' | 'amount' | 'progress'>('date')
const sortDir = ref<'asc' | 'desc'>('desc')

async function loadOrders() {
  await fetchOrders()
}

const hasWallet = computed(() => !!props.evmAddress || !!props.svmAddress)

const displayOrders = computed(() => {
  let result: TrackedOrder[] = []

  if (showMyOrdersOnly.value && hasWallet.value) {
    const evmOrders = props.evmAddress ? getOrdersBySender(props.evmAddress) : []
    const svmOrders = props.svmAddress ? getOrdersBySender(props.svmAddress) : []
    const combined = [...evmOrders, ...svmOrders]
    const seen = new Set<string>()
    result = combined.filter(order => {
      if (seen.has(order.order_id)) return false
      seen.add(order.order_id)
      return true
    })
  } else {
    result = [...orders.value]
  }

  // Sort orders
  result.sort((a, b) => {
    let comparison = 0
    switch (sortBy.value) {
      case 'amount':
        comparison = parseInt(a.amount_in) - parseInt(b.amount_in)
        break
      case 'progress':
        comparison = getFillPercentage(a) - getFillPercentage(b)
        break
      case 'date':
      default:
        comparison = (a.fill_deadline || 0) - (b.fill_deadline || 0)
    }
    return sortDir.value === 'asc' ? comparison : -comparison
  })

  return result
})

function selectOrder(order: TrackedOrder) {
  emit('select-order', order.order_id)
}

function truncateAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 4)}…${address.slice(-6)}`
}

function formatAmount(amount: string): string {
  const num = parseInt(amount) / 10**6
  if (isNaN(num)) return amount
  return num.toLocaleString(undefined, { maximumFractionDigits: 5 })
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

function getStatusInfo(order: TrackedOrder): { label: string; color: string; bg: string } {
  const pct = getFillPercentage(order)
  if (pct >= 100) return { label: 'Filled', color: '#22c55e', bg: 'rgba(34, 197, 94, 0.1)' }
  if (pct > 0) return { label: 'Partial', color: '#f59e0b', bg: 'rgba(245, 158, 11, 0.1)' }
  return { label: 'Open', color: '#6366f1', bg: 'rgba(99, 102, 241, 0.1)' }
}

function getChainName(chainId: number): string {
  if (chainId === undefined || chainId === null || chainId === 0) return 'Unknown'
  const chains: Record<number, string> = {
    1: 'ETH',
    8453: 'Base',
    42161: 'Arb',
    421614: 'Arb Sep',
    11155111: 'Sepolia',
    84532: 'Base Sep',
    1399811149: 'Solana',
    1399811150: 'Sol Dev',
  }
  return chains[chainId] || `${chainId}`
}

function getChainColor(chainId: number): string {
  const colors: Record<number, string> = {
    1: '#627eea',
    8453: '#0052ff',
    42161: '#28a0f0',
    421614: '#28a0f0',
    11155111: '#627eea',
    84532: '#0052ff',
    1399811149: '#9945ff',
    1399811150: '#9945ff',
  }
  return colors[chainId] || '#64748b'
}

function toggleSort(field: 'date' | 'amount' | 'progress') {
  if (sortBy.value === field) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortBy.value = field
    sortDir.value = 'desc'
  }
}

onMounted(loadOrders)
</script>

<template>
  <div class="glass-card orders-container">
    <!-- Header -->
    <div class="orders-header">
      <div class="header-left">
        <h2 class="title">Orders</h2>
        <span class="order-count" v-if="displayOrders.length > 0">{{ displayOrders.length }}</span>
      </div>

      <div class="header-actions">
        <!-- View Toggle -->
        <div class="view-toggle">
          <button
            @click="viewMode = 'table'"
            :class="['toggle-btn', viewMode === 'table' && 'active']"
            title="Table view"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 10h18M3 14h18M3 6h18M3 18h18"/>
            </svg>
          </button>
          <button
            @click="viewMode = 'grid'"
            :class="['toggle-btn', viewMode === 'grid' && 'active']"
            title="Grid view"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="7" height="7" rx="1"/>
              <rect x="14" y="3" width="7" height="7" rx="1"/>
              <rect x="3" y="14" width="7" height="7" rx="1"/>
              <rect x="14" y="14" width="7" height="7" rx="1"/>
            </svg>
          </button>
        </div>

        <!-- Filter Toggle -->
        <label class="filter-toggle" :class="{ disabled: !hasWallet }">
          <input
            type="checkbox"
            v-model="showMyOrdersOnly"
            :disabled="!hasWallet"
          />
          <span class="toggle-track">
            <span class="toggle-thumb"></span>
          </span>
          <span class="toggle-label">My orders</span>
        </label>

        <!-- Refresh -->
        <button @click="loadOrders" :disabled="loading" class="refresh-btn">
          <svg :class="['icon', loading && 'spin']" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- Loading State -->
    <div v-if="loading" class="state-container">
      <div class="loader"></div>
      <span class="state-text">Loading orders...</span>
    </div>

    <!-- Error State -->
    <div v-else-if="error" class="state-container error">
      <svg class="state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
      </svg>
      <span class="state-text">{{ error }}</span>
    </div>

    <!-- Empty State -->
    <div v-else-if="displayOrders.length === 0" class="state-container">
      <svg class="state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
      </svg>
      <span class="state-text">No orders found</span>
      <span class="state-subtext">Orders will appear here once created</span>
    </div>

    <!-- Table View -->
    <div v-else-if="viewMode === 'table'" class="table-container">
      <table class="orders-table">
        <thead>
          <tr>
            <th class="col-id">Order</th>
            <th class="col-route">Route</th>
            <th class="col-amount sortable" @click="toggleSort('amount')">
              <span>Amount</span>
              <svg v-if="sortBy === 'amount'" :class="['sort-icon', sortDir]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M19 9l-7 7-7-7"/>
              </svg>
            </th>
            <th class="col-receive">Receive</th>
            <th class="col-progress sortable" @click="toggleSort('progress')">
              <span>Progress</span>
              <svg v-if="sortBy === 'progress'" :class="['sort-icon', sortDir]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M19 9l-7 7-7-7"/>
              </svg>
            </th>
            <th class="col-status">Status</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="order in displayOrders"
            :key="order.order_id"
            @click="selectOrder(order)"
            class="order-row"
          >
            <td class="col-id">
              <span class="order-id">{{ truncateAddress(order.order_id) }}</span>
            </td>
            <td class="col-route">
              <div class="route-flow">
                <span class="chain-badge" :style="{ '--chain-color': getChainColor(order.origin_chain_id) }">
                  {{ getChainName(order.origin_chain_id) }}
                </span>
                <svg class="route-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M5 12h14m-4-4l4 4-4 4"/>
                </svg>
                <span class="chain-badge" :style="{ '--chain-color': getChainColor(order.dest_chain_id) }">
                  {{ getChainName(order.dest_chain_id) }}
                </span>
              </div>
            </td>
            <td class="col-amount">
              <div class="amount-cell">
                <span class="amount-value">{{ formatAmount(order.amount_in) }}</span>
                <span class="amount-token">{{ getTokenTicker(order.token_in) }}</span>
              </div>
            </td>
            <td class="col-receive">
              <div class="amount-cell">
                <span class="amount-value">{{ formatAmount(order.amount_out) }}</span>
                <span class="amount-token">{{ getTokenTicker(order.token_out) }}</span>
              </div>
            </td>
            <td class="col-progress">
              <div class="progress-cell">
                <div class="progress-bar">
                  <div
                    class="progress-fill"
                    :style="{
                      width: `${getFillPercentage(order)}%`,
                      '--progress-color': getStatusInfo(order).color
                    }"
                  ></div>
                </div>
                <span class="progress-text">{{ getFillPercentage(order).toFixed(0) }}%</span>
              </div>
            </td>
            <td class="col-status">
              <span
                class="status-badge"
                :style="{
                  '--status-color': getStatusInfo(order).color,
                  '--status-bg': getStatusInfo(order).bg
                }"
              >
                {{ getStatusInfo(order).label }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Grid View -->
    <div v-else class="grid-container">
      <div
        v-for="order in displayOrders"
        :key="order.order_id"
        @click="selectOrder(order)"
        class="order-card"
      >
        <!-- Card Header -->
        <div class="card-header">
          <span class="card-id">{{ truncateAddress(order.order_id) }}</span>
          <span
            class="status-badge"
            :style="{
              '--status-color': getStatusInfo(order).color,
              '--status-bg': getStatusInfo(order).bg
            }"
          >
            {{ getStatusInfo(order).label }}
          </span>
        </div>

        <!-- Route -->
        <div class="card-route">
          <div class="route-endpoint">
            <span class="chain-badge" :style="{ '--chain-color': getChainColor(order.origin_chain_id) }">
              {{ getChainName(order.origin_chain_id) }}
            </span>
          </div>
          <svg class="route-arrow-lg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M5 12h14m-4-4l4 4-4 4"/>
          </svg>
          <div class="route-endpoint">
            <span class="chain-badge" :style="{ '--chain-color': getChainColor(order.dest_chain_id) }">
              {{ getChainName(order.dest_chain_id) }}
            </span>
          </div>
        </div>

        <!-- Amounts -->
        <div class="card-amounts">
          <div class="amount-block">
            <span class="amount-label">Send</span>
            <span class="amount-value-lg">{{ formatAmount(order.amount_in) }}</span>
            <span class="amount-token-lg">{{ getTokenTicker(order.token_in) }}</span>
          </div>
          <div class="amount-block">
            <span class="amount-label">Receive</span>
            <span class="amount-value-lg">{{ formatAmount(order.amount_out) }}</span>
            <span class="amount-token-lg">{{ getTokenTicker(order.token_out) }}</span>
          </div>
        </div>

        <!-- Progress -->
        <div class="card-progress">
          <div class="progress-header">
            <span class="progress-label">Fill Progress</span>
            <span class="progress-pct" :style="{ color: getStatusInfo(order).color }">
              {{ getFillPercentage(order).toFixed(1) }}%
            </span>
          </div>
          <div class="progress-bar-lg">
            <div
              class="progress-fill"
              :style="{
                width: `${getFillPercentage(order)}%`,
                '--progress-color': getStatusInfo(order).color
              }"
            ></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.orders-container {
  border-radius: 24px;
  padding: 20px;
  max-height: calc(100vh - 180px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Header */
.orders-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.title {
  font-size: 18px;
  font-weight: 600;
  color: #fff;
  margin: 0;
  letter-spacing: -0.01em;
}

.order-count {
  font-size: 12px;
  font-weight: 500;
  color: rgba(148, 163, 184, 0.8);
  background: rgba(255, 255, 255, 0.05);
  padding: 2px 8px;
  border-radius: 10px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* View Toggle */
.view-toggle {
  display: flex;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 8px;
  padding: 2px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}

.toggle-btn {
  padding: 6px 8px;
  background: transparent;
  border: none;
  color: rgba(148, 163, 184, 0.6);
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.15s ease;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toggle-btn:hover {
  color: rgba(148, 163, 184, 0.9);
}

.toggle-btn.active {
  background: rgba(99, 102, 241, 0.15);
  color: #818cf8;
}

/* Filter Toggle */
.filter-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
}

.filter-toggle.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.filter-toggle input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.toggle-track {
  width: 32px;
  height: 18px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 9px;
  position: relative;
  transition: background 0.2s ease;
}

.filter-toggle input:checked + .toggle-track {
  background: rgba(99, 102, 241, 0.4);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  background: #94a3b8;
  border-radius: 50%;
  transition: all 0.2s ease;
}

.filter-toggle input:checked + .toggle-track .toggle-thumb {
  left: 16px;
  background: #818cf8;
}

.toggle-label {
  font-size: 12px;
  color: rgba(148, 163, 184, 0.8);
}

/* Refresh Button */
.refresh-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  color: rgba(148, 163, 184, 0.7);
  cursor: pointer;
  transition: all 0.15s ease;
}

.refresh-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
}

.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.refresh-btn .icon {
  width: 16px;
  height: 16px;
}

.refresh-btn .icon.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* State Containers */
.state-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 40px 20px;
}

.state-container.error {
  color: #f87171;
}

.state-icon {
  width: 40px;
  height: 40px;
  color: rgba(148, 163, 184, 0.4);
}

.state-container.error .state-icon {
  color: #f87171;
}

.state-text {
  font-size: 14px;
  color: rgba(148, 163, 184, 0.8);
}

.state-subtext {
  font-size: 12px;
  color: rgba(148, 163, 184, 0.5);
}

.loader {
  width: 28px;
  height: 28px;
  border: 2px solid rgba(99, 102, 241, 0.2);
  border-top-color: #818cf8;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

/* Table View */
.table-container {
  flex: 1;
  overflow: auto;
  margin: 0 -20px;
  padding: 0 20px;
}

.orders-table {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
}

.orders-table th {
  position: sticky;
  top: 0;
  background: rgba(15, 18, 25, 0.98);
  backdrop-filter: blur(8px);
  padding: 10px 12px;
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: rgba(148, 163, 184, 0.6);
  text-align: left;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  white-space: nowrap;
  z-index: 1;
}

.orders-table th.sortable {
  cursor: pointer;
  user-select: none;
}

.orders-table th.sortable:hover {
  color: rgba(148, 163, 184, 0.9);
}

.orders-table th span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.sort-icon {
  width: 12px;
  height: 12px;
  transition: transform 0.15s ease;
}

.sort-icon.asc {
  transform: rotate(180deg);
}

.order-row {
  cursor: pointer;
  transition: background 0.15s ease;
}

.order-row:hover {
  background: rgba(255, 255, 255, 0.02);
}

.order-row td {
  padding: 12px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  vertical-align: middle;
}

/* Column widths */
.col-id { width: 120px; }
.col-route { width: 180px; }
.col-amount { width: 140px; }
.col-receive { width: 140px; }
.col-progress { width: 140px; }
.col-status { width: 80px; }

.order-id {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 12px;
  color: rgba(148, 163, 184, 0.7);
}

/* Route */
.route-flow {
  display: flex;
  align-items: center;
  gap: 6px;
}

.chain-badge {
  font-size: 11px;
  font-weight: 500;
  padding: 3px 8px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--chain-color) 12%, transparent);
  color: var(--chain-color);
  border: 1px solid color-mix(in srgb, var(--chain-color) 20%, transparent);
  white-space: nowrap;
}

.route-arrow {
  width: 14px;
  height: 14px;
  color: rgba(148, 163, 184, 0.3);
  flex-shrink: 0;
}

/* Amounts */
.amount-cell {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.amount-value {
  font-size: 13px;
  font-weight: 500;
  color: #fff;
  font-variant-numeric: tabular-nums;
}

.amount-token {
  font-size: 11px;
  color: rgba(148, 163, 184, 0.6);
  font-weight: 500;
}

/* Progress */
.progress-cell {
  display: flex;
  align-items: center;
  gap: 10px;
}

.progress-bar {
  flex: 1;
  height: 4px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--progress-color);
  border-radius: 2px;
  transition: width 0.4s ease;
  box-shadow: 0 0 8px color-mix(in srgb, var(--progress-color) 40%, transparent);
}

.progress-text {
  font-size: 12px;
  font-weight: 500;
  color: rgba(148, 163, 184, 0.8);
  width: 36px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

/* Status Badge */
.status-badge {
  font-size: 11px;
  font-weight: 500;
  padding: 4px 10px;
  border-radius: 6px;
  background: var(--status-bg);
  color: var(--status-color);
  white-space: nowrap;
}

/* Grid View */
.grid-container {
  flex: 1;
  overflow: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
  padding: 4px;
}

.order-card {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 14px;
  padding: 16px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.order-card:hover {
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(255, 255, 255, 0.08);
  transform: translateY(-1px);
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}

.card-id {
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 11px;
  color: rgba(148, 163, 184, 0.6);
}

/* Card Route */
.card-route {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  margin-bottom: 16px;
  padding: 10px;
  background: rgba(0, 0, 0, 0.2);
  border-radius: 10px;
}

.route-endpoint {
  display: flex;
  align-items: center;
}

.route-arrow-lg {
  width: 18px;
  height: 18px;
  color: rgba(148, 163, 184, 0.4);
}

/* Card Amounts */
.card-amounts {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-bottom: 14px;
}

.amount-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.amount-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: rgba(148, 163, 184, 0.5);
}

.amount-value-lg {
  font-size: 16px;
  font-weight: 600;
  color: #fff;
  font-variant-numeric: tabular-nums;
}

.amount-token-lg {
  font-size: 12px;
  color: rgba(148, 163, 184, 0.6);
  font-weight: 500;
}

/* Card Progress */
.card-progress {
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.progress-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.progress-label {
  font-size: 11px;
  color: rgba(148, 163, 184, 0.5);
}

.progress-pct {
  font-size: 13px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.progress-bar-lg {
  height: 6px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 3px;
  overflow: hidden;
}

/* Scrollbar */
.table-container::-webkit-scrollbar,
.grid-container::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.table-container::-webkit-scrollbar-track,
.grid-container::-webkit-scrollbar-track {
  background: transparent;
}

.table-container::-webkit-scrollbar-thumb,
.grid-container::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 3px;
}

.table-container::-webkit-scrollbar-thumb:hover,
.grid-container::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.15);
}

/* Responsive */
@media (max-width: 768px) {
  .orders-container {
    padding: 16px;
    border-radius: 16px;
  }

  .header-actions {
    gap: 8px;
  }

  .filter-toggle .toggle-label {
    display: none;
  }

  .grid-container {
    grid-template-columns: 1fr;
  }

  .col-id, .col-status {
    display: none;
  }
}
</style>

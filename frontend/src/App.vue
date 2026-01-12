<script setup lang="ts">
import { ref, computed } from 'vue'
import NetworkSelector from './components/NetworkSelector.vue'
import WalletConnect from './components/WalletConnect.vue'
import SwapWidget from './components/SwapWidget.vue'
import OrdersPage from './components/OrdersPage.vue'
import OrderDetail from './components/OrderDetail.vue'

const network = ref<'local' | 'devnet' | 'mainnet'>(
  (import.meta.env.VITE_NETWORK as 'local' | 'devnet' | 'mainnet') || 'local'
)
const evmConnected = ref(false)
const svmConnected = ref(false)
const evmAddress = ref<string | null>(null)

const isConnected = computed(() => evmConnected.value || svmConnected.value)

// Navigation state
type Tab = 'swap' | 'orders'
const activeTab = ref<Tab>('swap')
const selectedOrderId = ref<string | null>(null)

function selectOrder(orderId: string) {
  selectedOrderId.value = orderId
}

function backToOrders() {
  selectedOrderId.value = null
}
</script>

<template>
  <div class="min-h-screen flex flex-col">
    <!-- Header -->
    <header class="border-b border-gray-800 px-6 py-4">
      <div class="max-w-7xl mx-auto flex items-center justify-between">
        <div class="flex items-center gap-4">
          <h1 class="text-xl font-bold text-primary-400">Liquidity Delivery</h1>
          <NetworkSelector v-model="network" />
        </div>
        <WalletConnect
          :network="network"
          @evm-connected="evmConnected = $event"
          @svm-connected="svmConnected = $event"
          @evm-address="evmAddress = $event"
        />
      </div>
    </header>

    <!-- Tabs -->
    <nav class="border-b border-gray-800">
      <div class="max-w-7xl mx-auto px-6">
        <div class="flex gap-1">
          <button
            @click="activeTab = 'swap'; selectedOrderId = null"
            :class="[
              'px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px',
              activeTab === 'swap'
                ? 'text-primary-400 border-primary-400'
                : 'text-gray-400 border-transparent hover:text-gray-300'
            ]"
          >
            Swap
          </button>
          <button
            @click="activeTab = 'orders'; selectedOrderId = null"
            :class="[
              'px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px',
              activeTab === 'orders'
                ? 'text-primary-400 border-primary-400'
                : 'text-gray-400 border-transparent hover:text-gray-300'
            ]"
          >
            Orders
          </button>
        </div>
      </div>
    </nav>

    <!-- Main Content -->
    <main class="flex-1 flex items-center justify-center p-6">
      <div class="w-full max-w-md">
        <!-- Swap Tab -->
        <SwapWidget
          v-if="activeTab === 'swap'"
          :network="network"
          :connected="isConnected"
        />

        <!-- Orders Tab -->
        <template v-else-if="activeTab === 'orders'">
          <OrderDetail
            v-if="selectedOrderId"
            :order-id="selectedOrderId"
            @back="backToOrders"
          />
          <OrdersPage
            v-else
            :wallet-address="evmAddress"
            @select-order="selectOrder"
          />
        </template>
      </div>
    </main>

    <!-- Footer -->
    <footer class="border-t border-gray-800 px-6 py-4">
      <div class="max-w-7xl mx-auto text-center text-gray-500 text-sm">
        <p>Cross-chain liquidity powered by M0</p>
      </div>
    </footer>
  </div>
</template>

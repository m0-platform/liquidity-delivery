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
const svmAddress = ref<string | null>(null)

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
  <div class="min-h-screen flex flex-col relative">
    <!-- Ambient background effects -->
    <div class="fixed inset-0 pointer-events-none overflow-hidden">
      <div class="absolute top-0 left-1/4 w-96 h-96 bg-accent-500/10 rounded-full blur-3xl animate-pulse-slow"></div>
      <div class="absolute bottom-1/4 right-1/4 w-80 h-80 bg-warm-500/5 rounded-full blur-3xl animate-pulse-slow" style="animation-delay: 1s;"></div>
    </div>

    <!-- Header -->
    <header class="relative z-10 border-b border-white/5 backdrop-blur-xl bg-slate-950/50">
      <div class="max-w-7xl mx-auto px-6 py-4">
        <div class="flex items-center justify-between">
          <!-- Logo & Network -->
          <div class="flex items-center gap-6">
            <!-- Logo -->
            <div class="flex items-center gap-3">
              <div class="relative">
                <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-accent-400 to-accent-600 flex items-center justify-center shadow-glow">
                  <svg class="w-6 h-6 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M12 2L2 7l10 5 10-5-10-5z" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M2 17l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
                    <path d="M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                </div>
                <div class="absolute -inset-1 rounded-xl bg-accent-500/20 blur-lg -z-10"></div>
              </div>
              <div>
                <h1 class="text-lg font-semibold text-white tracking-tight">Liquidity Delivery</h1>
                <p class="text-xs text-surface-500">Cross-chain transfers</p>
              </div>
            </div>

            <!-- Divider -->
            <div class="h-8 w-px bg-white/10"></div>

            <!-- Network Selector -->
            <NetworkSelector v-model="network" />
          </div>

          <!-- Wallet -->
          <WalletConnect
            :network="network"
            @evm-connected="evmConnected = $event"
            @svm-connected="svmConnected = $event"
            @evm-address="evmAddress = $event"
            @svm-address="svmAddress = $event"
          />
        </div>
      </div>
    </header>

    <!-- Navigation Tabs -->
    <nav class="relative z-10 border-b border-white/5 bg-slate-950/30 backdrop-blur-sm">
      <div class="max-w-7xl mx-auto px-6">
        <div class="flex gap-1">
          <button
            @click="activeTab = 'swap'; selectedOrderId = null"
            :class="[
              'relative px-5 py-3.5 text-sm font-medium transition-all duration-200',
              activeTab === 'swap'
                ? 'text-white'
                : 'text-surface-400 hover:text-surface-200'
            ]"
          >
            <span class="relative z-10 flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M7 16V4M7 4L3 8M7 4l4 4M17 8v12M17 20l4-4M17 20l-4-4" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Swap
            </span>
            <span
              v-if="activeTab === 'swap'"
              class="absolute bottom-0 left-0 right-0 h-0.5 bg-gradient-to-r from-accent-400 to-accent-500"
            ></span>
          </button>

          <button
            @click="activeTab = 'orders'; selectedOrderId = null"
            :class="[
              'relative px-5 py-3.5 text-sm font-medium transition-all duration-200',
              activeTab === 'orders'
                ? 'text-white'
                : 'text-surface-400 hover:text-surface-200'
            ]"
          >
            <span class="relative z-10 flex items-center gap-2">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              Orders
            </span>
            <span
              v-if="activeTab === 'orders'"
              class="absolute bottom-0 left-0 right-0 h-0.5 bg-gradient-to-r from-accent-400 to-accent-500"
            ></span>
          </button>
        </div>
      </div>
    </nav>

    <!-- Main Content -->
    <main class="flex-1 flex items-start justify-center px-6 py-10 relative z-10">
      <div class="w-full max-w-md animate-in">
        <!-- Swap Tab -->
        <SwapWidget
          v-if="activeTab === 'swap'"
          :network="network"
          :connected="isConnected"
          :evm-address="evmAddress"
          :svm-address="svmAddress"
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
    <footer class="relative z-10 border-t border-white/5 bg-slate-950/30 backdrop-blur-sm px-6 py-5">
      <p class="text-accent-400 text-sm w-full text-center">
        This is a development tool and should not be used in production
      </p>
    </footer>
  </div>
</template>

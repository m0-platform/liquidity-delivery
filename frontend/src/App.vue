<script setup lang="ts">
import { ref, computed } from 'vue'
import NetworkSelector from './components/NetworkSelector.vue'
import WalletConnect from './components/WalletConnect.vue'
import SwapWidget from './components/SwapWidget.vue'

const network = ref<'local' | 'devnet' | 'mainnet'>(
  (import.meta.env.VITE_NETWORK as 'local' | 'devnet' | 'mainnet') || 'local'
)
const evmConnected = ref(false)
const svmConnected = ref(false)

const isConnected = computed(() => evmConnected.value || svmConnected.value)
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
        />
      </div>
    </header>

    <!-- Main Content -->
    <main class="flex-1 flex items-center justify-center p-6">
      <div class="w-full max-w-md">
        <SwapWidget
          :network="network"
          :connected="isConnected"
        />
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

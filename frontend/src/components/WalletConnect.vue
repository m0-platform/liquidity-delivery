<script setup lang="ts">
import { ref, watch } from 'vue'
import { useWallet } from '../composables/useWallet'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
}>()

const emit = defineEmits<{
  'evm-connected': [connected: boolean]
  'svm-connected': [connected: boolean]
}>()

const {
  evmAddress,
  svmAddress,
  evmConnected,
  svmConnected,
  connectEvm,
  connectSvm,
  disconnectEvm,
  disconnectSvm,
  error
} = useWallet(props.network)

watch(evmConnected, (val) => emit('evm-connected', val))
watch(svmConnected, (val) => emit('svm-connected', val))

const showDropdown = ref(false)

function truncateAddress(addr: string): string {
  if (!addr) return ''
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`
}
</script>

<template>
  <div class="relative">
    <!-- Connected State -->
    <div v-if="evmConnected || svmConnected" class="flex items-center gap-2">
      <div v-if="evmAddress" class="flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
        <span class="w-2 h-2 rounded-full bg-blue-500"></span>
        <span class="text-sm">{{ truncateAddress(evmAddress) }}</span>
        <button
          @click="disconnectEvm"
          class="text-gray-400 hover:text-white ml-1"
        >
          ×
        </button>
      </div>

      <div v-if="svmAddress" class="flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
        <span class="w-2 h-2 rounded-full bg-purple-500"></span>
        <span class="text-sm">{{ truncateAddress(svmAddress) }}</span>
        <button
          @click="disconnectSvm"
          class="text-gray-400 hover:text-white ml-1"
        >
          ×
        </button>
      </div>

      <button
        v-if="!evmConnected || !svmConnected"
        @click="showDropdown = !showDropdown"
        class="bg-primary-600 hover:bg-primary-500 text-white px-4 py-2 rounded-lg text-sm font-medium"
      >
        +
      </button>
    </div>

    <!-- Disconnected State -->
    <button
      v-else
      @click="showDropdown = !showDropdown"
      class="bg-primary-600 hover:bg-primary-500 text-white px-4 py-2 rounded-lg text-sm font-medium"
    >
      Connect Wallet
    </button>

    <!-- Dropdown -->
    <div
      v-if="showDropdown"
      class="absolute right-0 top-full mt-2 w-56 bg-gray-800 rounded-lg shadow-xl border border-gray-700 overflow-hidden z-50"
    >
      <button
        v-if="!evmConnected"
        @click="connectEvm(); showDropdown = false"
        class="w-full px-4 py-3 text-left hover:bg-gray-700 flex items-center gap-3"
      >
        <span class="w-8 h-8 bg-blue-500 rounded-full flex items-center justify-center text-xs font-bold">
          MM
        </span>
        <span>MetaMask (EVM)</span>
      </button>

      <button
        v-if="!svmConnected"
        @click="connectSvm(); showDropdown = false"
        class="w-full px-4 py-3 text-left hover:bg-gray-700 flex items-center gap-3"
      >
        <span class="w-8 h-8 bg-purple-500 rounded-full flex items-center justify-center text-xs font-bold">
          PH
        </span>
        <span>Phantom (Solana)</span>
      </button>
    </div>

    <!-- Error Display -->
    <div
      v-if="error"
      class="absolute right-0 top-full mt-2 w-64 bg-red-900 text-red-200 rounded-lg p-3 text-sm"
    >
      {{ error }}
    </div>
  </div>
</template>

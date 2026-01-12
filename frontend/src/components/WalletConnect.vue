<script setup lang="ts">
import { ref, watch, toRef } from 'vue'
import { useWallet } from '../composables/useWallet'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
}>()

const emit = defineEmits<{
  'evm-connected': [connected: boolean]
  'svm-connected': [connected: boolean]
  'evm-address': [address: string | null]
}>()

// Pass network as a reactive ref so useWallet can react to changes
const networkRef = toRef(props, 'network')

const {
  evmAddress,
  svmAddress,
  evmConnected,
  svmConnected,
  isLocal,
  connect,
  disconnectEvm,
  disconnectSvm,
  error
} = useWallet(networkRef)

watch(evmConnected, (val) => emit('evm-connected', val))
watch(svmConnected, (val) => emit('svm-connected', val))
watch(evmAddress, (val) => emit('evm-address', val), { immediate: true })

const copied = ref<'evm' | 'svm' | null>(null)

function truncateAddress(addr: string): string {
  if (!addr) return ''
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`
}

async function copyAddress(addr: string, type: 'evm' | 'svm') {
  try {
    await navigator.clipboard.writeText(addr)
    copied.value = type
    setTimeout(() => {
      copied.value = null
    }, 2000)
  } catch (e) {
    console.error('Failed to copy:', e)
  }
}
</script>

<template>
  <div class="relative">
    <!-- Connected State -->
    <div v-if="evmConnected || svmConnected" class="flex items-center gap-2">
      <!-- EVM Wallet Display -->
      <div v-if="evmAddress" class="flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
        <span
          class="text-xs font-medium px-1.5 py-0.5 rounded"
          :class="isLocal ? 'bg-green-600 text-green-100' : 'bg-blue-600 text-blue-100'"
        >
          {{ isLocal ? 'Local' : 'EVM' }}
        </span>
        <span
          class="text-sm cursor-pointer hover:text-primary-400 transition-colors"
          :title="evmAddress"
          @click="copyAddress(evmAddress, 'evm')"
        >
          {{ truncateAddress(evmAddress) }}
        </span>
        <span v-if="copied === 'evm'" class="text-xs text-green-400">Copied!</span>
        <button
          @click="disconnectEvm"
          class="text-gray-400 hover:text-white ml-1 text-lg leading-none"
          title="Disconnect EVM wallet"
        >
          &times;
        </button>
      </div>

      <!-- SVM Wallet Display -->
      <div v-if="svmAddress" class="flex items-center gap-2 bg-gray-800 rounded-lg px-3 py-2">
        <span
          class="text-xs font-medium px-1.5 py-0.5 rounded"
          :class="isLocal ? 'bg-green-600 text-green-100' : 'bg-purple-600 text-purple-100'"
        >
          {{ isLocal ? 'Local' : 'SVM' }}
        </span>
        <span
          class="text-sm cursor-pointer hover:text-primary-400 transition-colors"
          :title="svmAddress"
          @click="copyAddress(svmAddress, 'svm')"
        >
          {{ truncateAddress(svmAddress) }}
        </span>
        <span v-if="copied === 'svm'" class="text-xs text-green-400">Copied!</span>
        <button
          @click="disconnectSvm"
          class="text-gray-400 hover:text-white ml-1 text-lg leading-none"
          title="Disconnect SVM wallet"
        >
          &times;
        </button>
      </div>

      <!-- Add Another Wallet Button (only for non-local mode when not both connected) -->
      <button
        v-if="!isLocal && (!evmConnected || !svmConnected)"
        @click="connect"
        class="bg-primary-600 hover:bg-primary-500 text-white px-3 py-2 rounded-lg text-sm font-medium"
        title="Connect another wallet"
      >
        +
      </button>
    </div>

    <!-- Disconnected State - directly open Reown modal for non-local -->
    <button
      v-else
      @click="connect"
      class="bg-primary-600 hover:bg-primary-500 text-white px-4 py-2 rounded-lg text-sm font-medium"
    >
      {{ isLocal ? 'Connect Local Wallets' : 'Connect Wallet' }}
    </button>

    <!-- Local Mode Info (only shown when disconnected in local mode) -->
    <div
      v-if="isLocal && !evmConnected && !svmConnected"
      class="absolute right-0 top-full mt-2 w-72 bg-gray-800 rounded-lg shadow-xl border border-gray-700 p-4 z-50"
    >
      <div class="flex items-start gap-2">
        <span class="text-yellow-400">&#9888;</span>
        <div class="text-sm">
          <p class="text-gray-300 mb-2">Local mode uses hardcoded private keys from environment variables.</p>
          <p class="text-gray-400 text-xs">Set <code class="bg-gray-700 px-1 rounded">VITE_LOCAL_EVM_PRIVATE_KEY</code> and <code class="bg-gray-700 px-1 rounded">VITE_LOCAL_SVM_PRIVATE_KEY</code> in your .env file.</p>
        </div>
      </div>
    </div>

    <!-- Error Display -->
    <div
      v-if="error"
      class="absolute right-0 top-full mt-2 w-72 bg-red-900/90 text-red-200 rounded-lg p-3 text-sm z-50 border border-red-700"
    >
      <div class="flex items-start gap-2">
        <span class="text-red-400">&#10006;</span>
        <span>{{ error }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, toRef } from 'vue'
import { useWallet } from '../composables/useWallet'

const props = defineProps<{
  network: 'local' | 'devnet' | 'mainnet'
}>()

const emit = defineEmits([
  'evm-connected',
  'svm-connected',
  'evm-address',
  'svm-address',
  'evm-signer',
  'svm-keypair',
  'solflare-wallet',
])

// Pass network as a reactive ref so useWallet can react to changes
const networkRef = toRef(props, 'network')

const {
  evmAddress,
  svmAddress,
  evmConnected,
  svmConnected,
  isLocal,
  connectEvm,
  connectSvm,
  disconnectEvm,
  disconnectSvm,
  localEvmWallet,
  localSvmKeypair,
  getSolflare,
  error
} = useWallet(networkRef)

watch(evmConnected, (val) => emit('evm-connected', val), { immediate: true })
watch(svmConnected, (val) => emit('svm-connected', val), { immediate: true })
watch(evmAddress, (val) => emit('evm-address', val), { immediate: true })
watch(svmAddress, (val) => emit('svm-address', val), { immediate: true })
watch(localEvmWallet, (val) => emit('evm-signer', val), { immediate: true })
watch(localSvmKeypair, (val) => emit('svm-keypair', val), { immediate: true })
// Emit Solflare instance whenever SVM connection state changes
watch(svmConnected, () => emit('solflare-wallet', getSolflare()), { immediate: true })

const copied = ref<'evm' | 'svm' | null>(null)
const showLocalInfo = ref(false)

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

function getWalletTypeColor(type: 'evm' | 'svm' | 'local'): string {
  const colors: Record<string, string> = {
    evm: 'from-blue-500 to-blue-600',
    svm: 'from-purple-500 to-purple-600',
    local: 'from-emerald-500 to-emerald-600',
  }
  return colors[type] || colors.evm
}
</script>

<template>
  <div class="relative">
    <!-- Always show both wallet slots -->
    <div class="flex items-center gap-2">
      <!-- EVM Wallet Slot -->
      <div
        v-if="evmAddress"
        class="group flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-accent-500/30 transition-all duration-200"
      >
        <!-- Chain Badge -->
        <div
          :class="[
            'w-6 h-6 rounded-lg flex items-center justify-center text-[10px] font-bold text-white bg-gradient-to-br',
            isLocal ? getWalletTypeColor('local') : getWalletTypeColor('evm')
          ]"
        >
          {{ isLocal ? 'L' : 'E' }}
        </div>

        <!-- Address -->
        <button
          class="text-sm font-mono text-surface-300 hover:text-white transition-colors"
          :title="evmAddress"
          @click="copyAddress(evmAddress, 'evm')"
        >
          {{ truncateAddress(evmAddress) }}
        </button>

        <!-- Copy Confirmation -->
        <Transition
          enter-active-class="transition-all duration-200"
          enter-from-class="opacity-0 scale-90"
          enter-to-class="opacity-100 scale-100"
          leave-active-class="transition-all duration-150"
          leave-from-class="opacity-100 scale-100"
          leave-to-class="opacity-0 scale-90"
        >
          <span v-if="copied === 'evm'" class="text-xs text-emerald-400 flex items-center gap-1">
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
              <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
        </Transition>

        <!-- Disconnect Button -->
        <button
          @click="disconnectEvm"
          class="text-surface-500 hover:text-rose-400 transition-colors p-1 -mr-1 rounded hover:bg-rose-500/10"
          title="Disconnect EVM wallet"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
      <!-- EVM Connect Button (when not connected) -->
      <button
        v-else
        @click="connectEvm"
        @mouseenter="isLocal && (showLocalInfo = true)"
        @mouseleave="showLocalInfo = false"
        class="flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-blue-500/30 transition-all duration-200 text-surface-400 hover:text-white"
      >
        <div class="w-6 h-6 rounded-lg flex items-center justify-center text-[10px] font-bold text-white bg-gradient-to-br from-blue-500 to-blue-600">
          E
        </div>
        <span class="text-sm">Connect EVM</span>
      </button>

      <!-- SVM Wallet Slot -->
      <div
        v-if="svmAddress"
        class="group flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-accent-500/30 transition-all duration-200"
      >
        <!-- Chain Badge -->
        <div
          :class="[
            'w-6 h-6 rounded-lg flex items-center justify-center text-[10px] font-bold text-white bg-gradient-to-br',
            isLocal ? getWalletTypeColor('local') : getWalletTypeColor('svm')
          ]"
        >
          {{ isLocal ? 'L' : 'S' }}
        </div>

        <!-- Address -->
        <button
          class="text-sm font-mono text-surface-300 hover:text-white transition-colors"
          :title="svmAddress"
          @click="copyAddress(svmAddress, 'svm')"
        >
          {{ truncateAddress(svmAddress) }}
        </button>

        <!-- Copy Confirmation -->
        <Transition
          enter-active-class="transition-all duration-200"
          enter-from-class="opacity-0 scale-90"
          enter-to-class="opacity-100 scale-100"
          leave-active-class="transition-all duration-150"
          leave-from-class="opacity-100 scale-100"
          leave-to-class="opacity-0 scale-90"
        >
          <span v-if="copied === 'svm'" class="text-xs text-emerald-400 flex items-center gap-1">
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
              <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </span>
        </Transition>

        <!-- Disconnect Button -->
        <button
          @click="disconnectSvm"
          class="text-surface-500 hover:text-rose-400 transition-colors p-1 -mr-1 rounded hover:bg-rose-500/10"
          title="Disconnect SVM wallet"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
      <!-- SVM Connect Button (when not connected) -->
      <button
        v-else
        @click="connectSvm"
        @mouseenter="isLocal && (showLocalInfo = true)"
        @mouseleave="showLocalInfo = false"
        class="flex items-center gap-2 bg-slate-850/80 rounded-xl px-3 py-2 border border-white/5 hover:border-purple-500/30 transition-all duration-200 text-surface-400 hover:text-white"
      >
        <div class="w-6 h-6 rounded-lg flex items-center justify-center text-[10px] font-bold text-white bg-gradient-to-br from-purple-500 to-purple-600">
          S
        </div>
        <span class="text-sm">Connect Solana</span>
      </button>
    </div>

    <!-- Local Mode Info Tooltip -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition-all duration-150 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-if="isLocal && showLocalInfo && !evmConnected && !svmConnected"
        class="absolute right-0 top-full mt-3 w-80 glass-card rounded-xl p-4 z-50"
      >
        <div class="flex items-start gap-3">
          <div class="w-8 h-8 rounded-lg bg-amber-500/20 flex items-center justify-center flex-shrink-0">
            <svg class="w-4 h-4 text-amber-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <div>
            <p class="text-sm text-surface-200 mb-2">Local mode uses hardcoded private keys from environment variables.</p>
            <div class="space-y-1">
              <code class="block text-xs bg-slate-800 text-accent-400 px-2 py-1 rounded font-mono">VITE_LOCAL_EVM_PRIVATE_KEY</code>
              <code class="block text-xs bg-slate-800 text-accent-400 px-2 py-1 rounded font-mono">VITE_LOCAL_SVM_PRIVATE_KEY</code>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Error Display -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition-all duration-150 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-if="error"
        class="absolute right-0 top-full mt-3 w-80 bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 z-50"
      >
        <div class="flex items-start gap-3">
          <svg class="w-5 h-5 flex-shrink-0 text-rose-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span class="text-sm">{{ error }}</span>
        </div>
      </div>
    </Transition>
  </div>
</template>

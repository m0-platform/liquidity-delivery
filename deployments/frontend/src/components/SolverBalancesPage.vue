<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useSolverBalances } from '../composables/useSolverBalances'
import { useAssets } from '../composables/useAssets'
import type { NetworkType } from '../config/network'

const props = defineProps<{
  network: NetworkType
}>()

const networkRef = computed(() => props.network)
const { balances, balancesByChain, loading, error, fetchBalances } = useSolverBalances(networkRef)
const { assets } = useAssets(networkRef)

function getTokenIcon(symbol: string, chain: string): string | null {
  const asset = assets.value.find(
    (a) => a.symbol.toUpperCase() === symbol.toUpperCase() && a.chain === chain
  )
  return asset?.icon || null
}

function formatBalance(balance: string, decimals: number): string {
  const num = parseFloat(balance) / Math.pow(10, decimals)
  if (isNaN(num)) return balance

  // Format with appropriate precision
  if (num >= 1000000) {
    return (num / 1000000).toFixed(2) + 'M'
  } else if (num >= 1000) {
    return (num / 1000).toFixed(2) + 'K'
  } else if (num < 0.01 && num > 0) {
    return num.toFixed(6)
  }
  return num.toLocaleString(undefined, { maximumFractionDigits: 4 })
}

function getChainColor(chain: string): string {
  const colors: Record<string, string> = {
    'Ethereum': '#627eea',
    'Base': '#0052ff',
    'Arbitrum': '#28a0f0',
    'Sepolia': '#627eea',
    'Base Sepolia': '#0052ff',
    'Solana': '#9945ff',
    'Solana Devnet': '#9945ff',
  }
  return colors[chain] || '#64748b'
}

function getChainGradient(chain: string): string {
  const gradients: Record<string, string> = {
    'Ethereum': 'from-[#627eea] to-[#4a5fc1]',
    'Base': 'from-[#0052ff] to-[#003acc]',
    'Arbitrum': 'from-[#28a0f0] to-[#1c7ac0]',
    'Sepolia': 'from-[#627eea] to-[#4a5fc1]',
    'Base Sepolia': 'from-[#0052ff] to-[#003acc]',
    'Solana': 'from-[#9945ff] to-[#14f195]',
    'Solana Devnet': 'from-[#9945ff] to-[#14f195]',
  }
  return gradients[chain] || 'from-slate-500 to-slate-600'
}

function truncateAddress(address: string): string {
  if (address.length <= 13) return address
  return `${address.slice(0, 6)}...${address.slice(-4)}`
}

onMounted(fetchBalances)
</script>

<template>
  <div class="glass-card rounded-3xl overflow-hidden">
    <!-- Header with gradient accent -->
    <div class="relative p-6 border-b border-white/5">
      <!-- Decorative background -->
      <div class="absolute inset-0 overflow-hidden pointer-events-none">
        <div class="absolute -top-20 -right-20 w-40 h-40 bg-accent-500/10 rounded-full blur-3xl"></div>
        <div class="absolute -bottom-10 -left-10 w-32 h-32 bg-warm-500/5 rounded-full blur-2xl"></div>
      </div>

      <div class="relative flex items-center justify-between">
        <div class="flex items-center gap-4">
          <!-- Icon -->
          <div class="relative">
            <div class="w-12 h-12 rounded-2xl bg-gradient-to-br from-accent-400/20 to-accent-600/20 flex items-center justify-center border border-accent-500/20">
              <svg class="w-6 h-6 text-accent-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M2 10h20M2 14h20M6 18h12M8 22h8M4 6h16M8 2h8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>
            <!-- Pulse ring -->
            <div class="absolute inset-0 rounded-2xl border border-accent-500/30 animate-ping opacity-20"></div>
          </div>

          <div>
            <h2 class="text-xl font-semibold text-white">Solver Inventory</h2>
            <p class="text-surface-500 text-sm mt-0.5">Available liquidity across chains</p>
          </div>
        </div>

        <!-- Refresh button -->
        <button
          @click="fetchBalances"
          :disabled="loading"
          class="btn-secondary flex items-center gap-2 text-sm"
        >
          <svg
            :class="['w-4 h-4 transition-transform', loading && 'animate-spin']"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Refresh
        </button>
      </div>
    </div>

    <!-- Content -->
    <div class="p-6">
      <!-- Loading State -->
      <div v-if="loading && balances.length === 0" class="py-16">
        <div class="flex flex-col items-center gap-4">
          <div class="relative">
            <div class="w-16 h-16 rounded-full border-2 border-accent-500/20 border-t-accent-500 animate-spin"></div>
            <div class="absolute inset-0 flex items-center justify-center">
              <div class="w-8 h-8 rounded-full bg-accent-500/10"></div>
            </div>
          </div>
          <span class="text-surface-400 text-sm">Loading inventory...</span>
        </div>
      </div>

      <!-- Error State -->
      <div v-else-if="error" class="bg-rose-500/10 border border-rose-500/20 text-rose-300 rounded-xl p-4 flex items-start gap-3">
        <svg class="w-5 h-5 flex-shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <div>
          <span class="text-sm font-medium">Failed to load balances</span>
          <p class="text-xs text-rose-400/80 mt-0.5">{{ error }}</p>
        </div>
      </div>

      <!-- Empty State -->
      <div v-else-if="balances.length === 0" class="py-16">
        <div class="flex flex-col items-center gap-4">
          <div class="w-20 h-20 rounded-2xl bg-slate-800/50 flex items-center justify-center">
            <svg class="w-10 h-10 text-surface-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <div class="text-center">
            <p class="text-surface-300 font-medium">No balances found</p>
            <p class="text-surface-500 text-sm mt-1">Solver inventory will appear here</p>
          </div>
        </div>
      </div>

      <!-- Balances by Chain -->
      <div v-else class="space-y-6">
        <TransitionGroup
          enter-active-class="transition-all duration-500 ease-out"
          enter-from-class="opacity-0 translate-y-4"
          enter-to-class="opacity-100 translate-y-0"
          leave-active-class="transition-all duration-300 ease-in"
          leave-from-class="opacity-100"
          leave-to-class="opacity-0"
        >
          <div
            v-for="([chain, chainBalances], chainIndex) in balancesByChain"
            :key="chain"
            class="relative"
            :style="{ animationDelay: `${chainIndex * 100}ms` }"
          >
            <!-- Chain Header -->
            <div class="flex items-center gap-3 mb-4">
              <div
                class="w-8 h-8 rounded-lg bg-gradient-to-br flex items-center justify-center shadow-lg"
                :class="getChainGradient(chain)"
              >
                <svg class="w-4 h-4 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </div>
              <div>
                <h3 class="text-sm font-semibold text-white">{{ chain }}</h3>
                <p class="text-xs text-surface-500">{{ chainBalances.length }} token{{ chainBalances.length === 1 ? '' : 's' }}</p>
              </div>

              <!-- Chain indicator line -->
              <div class="flex-1 h-px ml-2" :style="{ background: `linear-gradient(90deg, ${getChainColor(chain)}40 0%, transparent 100%)` }"></div>
            </div>

            <!-- Token Cards Grid -->
            <div class="grid gap-2">
              <div
                v-for="(balance, index) in chainBalances"
                :key="`${balance.chain}-${balance.address}`"
                class="group bg-slate-925/60 rounded-lg px-3 py-2 border border-white/5 hover:border-accent-500/20 transition-all duration-300"
                :style="{ animationDelay: `${(chainIndex * 100) + (index * 50)}ms` }"
              >
                <div class="flex items-center justify-between">
                  <!-- Token Info -->
                  <div class="flex items-center gap-2.5">
                    <!-- Token Icon -->
                    <img
                      v-if="getTokenIcon(balance.symbol, balance.chain)"
                      :src="getTokenIcon(balance.symbol, balance.chain)!"
                      :alt="balance.symbol"
                      class="w-7 h-7 rounded-full ring-1 ring-white/10"
                    />
                    <div
                      v-else
                      class="w-7 h-7 rounded-full flex items-center justify-center text-[10px] font-bold text-white ring-1 ring-white/10 bg-slate-700"
                    >
                      {{ balance.symbol.slice(0, 2) }}
                    </div>

                    <div class="flex items-center gap-2">
                      <span class="text-white font-medium text-sm">{{ balance.symbol }}</span>
                      <span
                        class="text-[10px] px-1.5 py-0.5 rounded bg-white/5 text-surface-500 font-mono"
                        :title="balance.address"
                      >
                        {{ truncateAddress(balance.address) }}
                      </span>
                    </div>
                  </div>

                  <!-- Balance -->
                  <div class="text-sm font-semibold text-white tabular-nums">
                    {{ formatBalance(balance.balance, balance.decimals) }}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </TransitionGroup>
      </div>
    </div>
  </div>
</template>

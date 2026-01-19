<script setup lang="ts">
defineProps<{
  modelValue: 'local' | 'devnet' | 'mainnet'
}>()

defineEmits<{
  'update:modelValue': [value: 'local' | 'devnet' | 'mainnet']
}>()

const networks = [
  { id: 'local', name: 'Local', icon: '🔧', description: 'Development' },
  { id: 'devnet', name: 'Devnet', icon: '🧪', description: 'Testing' },
  { id: 'mainnet', name: 'Mainnet', icon: '🌐', description: 'Production' },
] as const

function getNetworkColor(id: string, isActive: boolean): string {
  const colors: Record<string, { active: string; dot: string }> = {
    local: { active: 'border-emerald-500/40 shadow-[0_0_15px_-3px_rgba(16,185,129,0.3)]', dot: 'bg-emerald-500' },
    devnet: { active: 'border-amber-500/40 shadow-[0_0_15px_-3px_rgba(245,158,11,0.3)]', dot: 'bg-amber-500' },
    mainnet: { active: 'border-rose-500/40 shadow-[0_0_15px_-3px_rgba(244,63,94,0.3)]', dot: 'bg-rose-500' },
  }
  return isActive ? colors[id]?.active || '' : ''
}

function getDotColor(id: string): string {
  const colors: Record<string, string> = {
    local: 'bg-emerald-500',
    devnet: 'bg-amber-500',
    mainnet: 'bg-rose-500',
  }
  return colors[id] || 'bg-gray-500'
}
</script>

<template>
  <div class="flex items-center gap-1 p-1 bg-slate-900/50 rounded-xl border border-white/5">
    <button
      v-for="net in networks"
      :key="net.id"
      :class="[
        'relative px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200 border',
        modelValue === net.id
          ? ['bg-slate-800/80 text-white', getNetworkColor(net.id, true)]
          : 'bg-transparent text-surface-400 hover:text-surface-200 hover:bg-slate-800/40 border-transparent'
      ]"
      @click="$emit('update:modelValue', net.id)"
    >
      <span class="flex items-center gap-2">
        <span
          :class="[
            'w-2 h-2 rounded-full transition-all duration-300',
            getDotColor(net.id),
            modelValue === net.id && 'animate-pulse shadow-lg'
          ]"
          :style="modelValue === net.id ? `box-shadow: 0 0 8px ${net.id === 'local' ? 'rgba(16,185,129,0.6)' : net.id === 'devnet' ? 'rgba(245,158,11,0.6)' : 'rgba(244,63,94,0.6)'}` : ''"
        ></span>
        <span>{{ net.name }}</span>
      </span>
    </button>
  </div>
</template>

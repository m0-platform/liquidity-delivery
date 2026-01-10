<script setup lang="ts">
defineProps<{
  modelValue: 'local' | 'devnet' | 'mainnet'
}>()

defineEmits<{
  'update:modelValue': [value: 'local' | 'devnet' | 'mainnet']
}>()

const networks = [
  { id: 'local', name: 'Local', color: 'bg-green-500' },
  { id: 'devnet', name: 'Devnet', color: 'bg-yellow-500' },
  { id: 'mainnet', name: 'Mainnet', color: 'bg-red-500' },
] as const
</script>

<template>
  <div class="flex items-center gap-2">
    <button
      v-for="net in networks"
      :key="net.id"
      :class="[
        'px-3 py-1.5 rounded-lg text-sm font-medium transition-all',
        modelValue === net.id
          ? 'bg-gray-700 text-white'
          : 'bg-gray-800 text-gray-400 hover:bg-gray-700 hover:text-gray-200'
      ]"
      @click="$emit('update:modelValue', net.id)"
    >
      <span class="flex items-center gap-2">
        <span :class="['w-2 h-2 rounded-full', net.color]"></span>
        {{ net.name }}
      </span>
    </button>
  </div>
</template>

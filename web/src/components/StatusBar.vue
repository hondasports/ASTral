<template>
  <div class="flex items-center gap-3 text-xs text-slate-400">
    <span v-if="loading" class="animate-pulse text-cyan-400">indexing...</span>
    <span v-else-if="status?.indexed" class="text-emerald-400">indexed</span>
    <span v-else-if="status?.error" class="text-red-400">{{ status.error }}</span>
    <span v-else class="text-slate-500">not indexed</span>
    <button
      @click="$emit('refresh')"
      class="rounded border border-slate-700 px-2 py-1 transition hover:border-cyan-500 hover:text-cyan-400"
    >
      refresh
    </button>
  </div>
</template>

<script setup>
defineProps(['status', 'loading'])
defineEmits(['refresh'])
</script>

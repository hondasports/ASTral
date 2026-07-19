<template>
  <div class="flex-1 flex items-center gap-2">
    <input
      v-model="repo"
      type="text"
      placeholder="repo name"
      class="w-40 rounded-md border border-slate-700 bg-slate-950 px-3 py-1.5 text-sm focus:border-cyan-500 focus:outline-none"
    />
    <select v-model="mode" class="rounded-md border border-slate-700 bg-slate-950 px-2 py-1.5 text-sm">
      <option value="code">全文検索</option>
      <option value="symbol">シンボル検索</option>
    </select>
    <input
      v-model="query"
      type="text"
      placeholder="query..."
      @keyup.enter="search"
      class="flex-1 rounded-md border border-slate-700 bg-slate-950 px-3 py-1.5 text-sm focus:border-cyan-500 focus:outline-none"
    />
    <button
      @click="search"
      class="rounded-md bg-gradient-to-r from-cyan-500 to-blue-600 px-4 py-1.5 text-sm font-semibold text-white shadow transition hover:from-cyan-400 hover:to-blue-500"
    >
      検索
    </button>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps(['repo', 'query', 'mode'])
const emit = defineEmits(['update:repo', 'update:query', 'update:mode', 'search'])

const repo = computed({
  get: () => props.repo,
  set: (v) => emit('update:repo', v),
})
const query = computed({
  get: () => props.query,
  set: (v) => emit('update:query', v),
})
const mode = computed({
  get: () => props.mode,
  set: (v) => emit('update:mode', v),
})

function search() {
  emit('search')
}
</script>

<template>
  <div class="h-full bg-slate-900 p-3">
    <h2 class="mb-2 text-xs font-bold uppercase tracking-wider text-slate-400">
      Results
    </h2>
    <div v-if="!results || results.length === 0" class="text-sm text-slate-500">
      結果はありません
    </div>
    <ul class="space-y-2">
      <li
        v-for="(item, i) in results"
        :key="i"
        @click="$emit('select', item)"
        class="cursor-pointer rounded-md border border-slate-800 bg-slate-950 p-2 text-sm transition hover:border-cyan-500 hover:shadow"
      >
        <div v-if="item.error" class="text-red-400">{{ item.error }}</div>
        <div v-else>
          <div class="font-semibold text-cyan-300">
            {{ item.label || item.name || item.relative_path }}
          </div>
          <div class="mt-1 truncate text-xs text-slate-400">
            {{ item.relative_path }}
          </div>
          <div v-if="item.snippet || item.content" class="mt-1 line-clamp-3 text-xs text-slate-300">
            {{ item.snippet || item.content }}
          </div>
        </div>
      </li>
    </ul>
  </div>
</template>

<script setup>
defineProps(['results', 'mode'])
defineEmits(['select'])
</script>

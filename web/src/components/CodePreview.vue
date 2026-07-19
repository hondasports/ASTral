<template>
  <div class="flex h-full flex-col bg-slate-950">
    <div class="flex-none border-b border-slate-800 px-4 py-3">
      <h2 class="text-sm font-bold text-cyan-300">
        {{ symbol?.name || 'Preview' }}
      </h2>
      <div class="text-xs text-slate-400">
        {{ symbol?.path || '' }}
        <span v-if="symbol?.kind">· {{ symbol.kind }}</span>
      </div>
    </div>
    <div class="relative flex-1 overflow-auto p-4">
      <div v-if="loading" class="text-sm text-slate-400">Loading...</div>
      <pre
        v-else-if="source"
        class="h-full"
      ><code ref="code" class="language-typescript rounded-lg text-xs">{{ source }}</code></pre>
      <div v-else class="text-sm text-slate-500">
        シンボルまたは検索結果を選択するとコードが表示されます
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, nextTick } from 'vue'
import hljs from 'highlight.js/lib/core'
import typescript from 'highlight.js/lib/languages/typescript'
import javascript from 'highlight.js/lib/languages/javascript'
import rust from 'highlight.js/lib/languages/rust'
import python from 'highlight.js/lib/languages/python'
import go from 'highlight.js/lib/languages/go'
import 'highlight.js/styles/atom-one-dark.css'

hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('python', python)
hljs.registerLanguage('go', go)

const props = defineProps(['symbol', 'source', 'loading'])
const code = ref(null)

watch(() => props.source, async () => {
  await nextTick()
  if (code.value) {
    hljs.highlightElement(code.value)
  }
}, { immediate: true })
</script>

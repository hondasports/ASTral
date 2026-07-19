<template>
  <div class="h-screen flex flex-col bg-slate-950 text-slate-100">
    <header class="flex-none border-b border-slate-800 bg-slate-900/80 backdrop-blur px-4 py-3">
      <div class="flex items-center gap-4">
        <h1 class="text-xl font-bold bg-gradient-to-r from-cyan-400 to-blue-500 bg-clip-text text-transparent">
          ASTral
        </h1>
        <SearchPanel
          v-model:repo="repo"
          v-model:query="query"
          v-model:mode="mode"
          @search="onSearch"
        />
        <StatusBar :status="status" :loading="loadingStatus" @refresh="refreshStatus" />
      </div>
    </header>
    <main class="flex-1 flex overflow-hidden">
      <ResultList
        class="w-80 flex-none border-r border-slate-800 overflow-y-auto"
        :results="results"
        :mode="mode"
        @select="onSelect"
      />
      <SymbolGraph
        class="flex-1 border-r border-slate-800"
        :data="graphData"
        :loading="loadingGraph"
        @select-node="onGraphNode"
      />
      <CodePreview
        class="w-[480px] flex-none"
        :symbol="selectedSymbol"
        :source="source"
        :loading="loadingSource"
      />
    </main>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { api } from './api.js'
import SearchPanel from './components/SearchPanel.vue'
import ResultList from './components/ResultList.vue'
import SymbolGraph from './components/SymbolGraph.vue'
import CodePreview from './components/CodePreview.vue'
import StatusBar from './components/StatusBar.vue'

const repo = ref('my-repo')
const query = ref('')
const mode = ref('code')
const results = ref([])
const selectedSymbol = ref(null)
const source = ref('')
const graphData = ref({ nodes: [], edges: [] })
const status = ref({})
const loadingStatus = ref(false)
const loadingGraph = ref(false)
const loadingSource = ref(false)

onMounted(() => {
  refreshStatus()
})

async function refreshStatus() {
  loadingStatus.value = true
  try {
    status.value = await api.status(repo.value)
  } catch (e) {
    status.value = { error: e.message }
  } finally {
    loadingStatus.value = false
  }
}

async function onSearch() {
  if (!query.value.trim()) return
  try {
    if (mode.value === 'code') {
      const res = await api.search(repo.value, query.value)
      results.value = res.results.map((r) => ({ ...r, type: 'search', label: `${r.relative_path}:${r.startByte}` }))
    } else {
      const res = await api.findSymbol(repo.value, query.value)
      results.value = res.results.map((r) => ({ ...r, type: 'symbol' }))
    }
  } catch (e) {
    results.value = [{ error: e.message }]
  }
}

async function onSelect(item) {
  if (item.error || !item) return
  if (item.type === 'search') {
    selectedSymbol.value = {
      name: item.label,
      path: item.relative_path,
      kind: 'search hit',
      symbolId: null,
    }
    source.value = item.snippet || ''
    graphData.value = { nodes: [], edges: [] }
  } else {
    selectedSymbol.value = {
      name: item.name,
      path: item.relative_path,
      kind: item.kind,
      symbolId: item.symbolId,
    }
    await loadSymbol(item.symbolId || item.name)
  }
}

async function onGraphNode(node) {
  if (!node?.id) return
  await loadSymbol(node.id)
}

async function loadSymbol(identifier) {
  loadingSource.value = true
  loadingGraph.value = true
  try {
    const [readRes, graphRes] = await Promise.all([
      api.readSymbol(repo.value, identifier).catch(() => null),
      api.graph(repo.value, identifier).catch(() => ({ nodes: [], edges: [] })),
    ])
    source.value = readRes?.source || ''
    selectedSymbol.value = {
      name: readRes?.name || identifier,
      path: readRes?.path || '',
      kind: readRes?.kind || '',
      symbolId: readRes?.symbolId || identifier,
    }
    graphData.value = graphRes
  } catch (e) {
    source.value = e.message
  } finally {
    loadingSource.value = false
    loadingGraph.value = false
  }
}
</script>

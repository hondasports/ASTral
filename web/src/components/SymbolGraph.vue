<template>
  <div class="relative h-full w-full bg-slate-900">
    <div ref="network" class="h-full w-full"></div>
    <div
      v-if="loading"
      class="pointer-events-none absolute inset-0 flex items-center justify-center"
    >
      <div class="rounded-lg bg-slate-800 px-4 py-2 text-sm text-slate-200 shadow">
        Loading graph...
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { Network, DataSet } from 'vis-network/standalone'
import 'vis-network/styles/vis-network.css'

const props = defineProps(['data', 'loading'])
const emit = defineEmits(['select-node'])
const network = ref(null)
let net = null

function colorForKind(kind) {
  switch (kind) {
    case 'function':
      return '#22d3ee'
    case 'class':
      return '#a78bfa'
    case 'method':
      return '#34d399'
    case 'variable':
      return '#fbbf24'
    case 'interface':
      return '#f472b6'
    default:
      return '#94a3b8'
  }
}

function buildNodes() {
  const nodes = new DataSet(
    (props.data?.nodes || []).map((n) => ({
      id: n.id,
      label: n.label,
      title: `${n.kind || 'symbol'} | ${n.path || ''}`,
      color: {
        background: n.isCenter ? '#f43f5e' : colorForKind(n.kind),
        border: '#e2e8f0',
        highlight: { background: '#f43f5e', border: '#fff' },
      },
      font: { color: '#e2e8f0', face: 'Inter' },
      shape: n.isCenter ? 'box' : 'dot',
      size: n.isCenter ? 22 : 14,
    }))
  )
  const edges = new DataSet(
    (props.data?.edges || []).map((e) => ({
      from: e.from,
      to: e.to,
      label: e.label,
      arrows: 'to',
      color: { color: '#64748b', highlight: '#38bdf8' },
      font: { color: '#94a3b8', size: 10, align: 'middle' },
      smooth: { type: 'continuous' },
    }))
  )
  return { nodes, edges }
}

function draw() {
  if (!network.value) return
  if (net) {
    net.destroy()
    net = null
  }

  const { nodes, edges } = buildNodes()
  const options = {
    nodes: {
      borderWidth: 2,
      shadow: true,
    },
    edges: {
      width: 1,
      shadow: true,
      smooth: true,
    },
    physics: {
      stabilization: { iterations: 200 },
      barnesHut: {
        gravitationalConstant: -3000,
        centralGravity: 0.3,
        springLength: 130,
        springConstant: 0.04,
      },
    },
    interaction: {
      hover: true,
      tooltipDelay: 100,
    },
  }

  net = new Network(network.value, { nodes, edges }, options)
  net.on('click', (params) => {
    if (params.nodes && params.nodes.length > 0) {
      const id = params.nodes[0]
      const node = (props.data?.nodes || []).find((n) => n.id === id)
      emit('select-node', node)
    }
  })
}

watch(() => props.data, draw, { deep: true })
onMounted(draw)
onUnmounted(() => {
  if (net) {
    net.destroy()
    net = null
  }
})
</script>

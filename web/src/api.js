const API_BASE = ''

function endpoint(path) {
  return `${API_BASE}${path}`
}

async function post(path, body) {
  const res = await fetch(endpoint(path), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`${res.status}: ${text}`)
  }
  return res.json()
}

async function get(path) {
  const res = await fetch(endpoint(path))
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`${res.status}: ${text}`)
  }
  return res.json()
}

export const api = {
  status(repo) {
    return get(`/api/status?repository_name=${encodeURIComponent(repo)}`)
  },
  search(repo, query, limit = 20) {
    return post('/api/search', { repository_name: repo, query, limit })
  },
  findSymbol(repo, query, limit = 20) {
    return post('/api/find-symbol', { repository_name: repo, query, limit })
  },
  readSymbol(repo, symbolId) {
    return post('/api/read-symbol', { repository_name: repo, symbol_id: symbolId })
  },
  graph(repo, symbol) {
    return post('/api/graph', { repository_name: repo, symbol })
  },
  refresh(repo) {
    return post('/api/refresh', { repository_name: repo })
  },
}

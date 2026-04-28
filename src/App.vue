<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { PaneInfo, SessionSummary, UsageDiagnostics } from './types'

const panes = ref<PaneInfo[]>([])
const usageSummary = ref<SessionSummary[]>([])
const usageDiagnostics = ref<UsageDiagnostics | null>(null)
const usageLoading = ref(true)
const activeTab = ref<'usage' | 'pending' | 'settings'>('usage')
const error = ref('')
let unlistenUsage: null | (() => void) = null
let usageRefreshInterval: ReturnType<typeof setInterval> | null = null

async function refreshPanes() {
  try {
    panes.value = await invoke<PaneInfo[]>('list_panes')
    error.value = ''
  } catch (e) {
    error.value = String(e)
    panes.value = []
  }
}

async function refreshUsage() {
  usageLoading.value = true
  try {
    usageSummary.value = await invoke<SessionSummary[]>('get_usage_summary')
    error.value = ''
  } catch (e) {
    error.value = String(e)
    usageSummary.value = []
  } finally {
    usageLoading.value = false
  }

  try {
    usageDiagnostics.value = await invoke<UsageDiagnostics>('get_usage_diagnostics')
  } catch {
    usageDiagnostics.value = null
  }
}

function refreshAll() {
  void refreshPanes()
  void refreshUsage()
}

function sessionLabel(item: SessionSummary) {
  if (item.session_title) {
    return item.session_title
  }

  if (item.session_id) {
    return `session ${item.session_id.slice(0, 8)}`
  }

  return 'unknown session'
}

onMounted(() => {
  void listen<SessionSummary[]>('usage:updated', (event) => {
      usageSummary.value = event.payload
      usageLoading.value = false
    })
    .then((unlisten) => {
      unlistenUsage = unlisten
    })

  refreshAll()
  usageRefreshInterval = setInterval(() => {
    void refreshUsage()
  }, 10_000)
})

onBeforeUnmount(() => {
  void unlistenUsage?.()
  if (usageRefreshInterval) {
    clearInterval(usageRefreshInterval)
  }
})
</script>

<template>
  <main class="shell">
    <header>
      <h1>AI Shepherd</h1>
      <button @click="refreshAll">Refresh</button>
    </header>

    <nav>
      <button @click="activeTab = 'usage'; refreshUsage()">Usage Monitor</button>
      <button @click="activeTab = 'pending'">Pending Prompts</button>
      <button @click="activeTab = 'settings'">Settings</button>
    </nav>

    <section v-if="activeTab === 'usage'">
      <h2>Usage Monitor</h2>
      <p v-if="usageLoading">Loading Claude Code usage from recent session files...</p>
      <p v-else-if="usageSummary.length === 0">
        No Claude Code usage files found yet.
        <span v-if="usageDiagnostics">
          Checked {{ usageDiagnostics.candidate_files }} JSONL files,
          parsed {{ usageDiagnostics.token_events }} token events.
        </span>
      </p>
      <ul v-else>
        <li v-for="(item, index) in usageSummary" :key="`${item.provider_id}:${item.session_id ?? 'unknown'}:${index}`">
          <strong>{{ item.provider_id }}</strong>
          <span> — {{ sessionLabel(item) }}</span>
          — {{ item.tokens_used }} tokens, {{ item.requests_used }} requests, ${{ item.cost.toFixed(2) }}
        </li>
      </ul>
    </section>
    <section v-else-if="activeTab === 'pending'">
      <h2>Pending Prompts</h2>
      <ul><li v-for="pane in panes" :key="pane.id">{{ pane.title }} ({{ pane.terminal_adapter }})</li></ul>
    </section>
    <section v-else>
      <h2>Settings</h2>
      <p>Placeholder.</p>
    </section>

    <p v-if="error">{{ error }}</p>
  </main>
</template>

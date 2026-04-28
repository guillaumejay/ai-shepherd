<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import type { PaneInfo, SessionSummary } from './types'

const panes = ref<PaneInfo[]>([])
const usageSummary = ref<SessionSummary[]>([])
const usageLoading = ref(false)
const activeTab = ref<'usage' | 'pending' | 'settings'>('usage')
const error = ref('')

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

onMounted(async () => {
  await Promise.all([refreshPanes(), refreshUsage()])
})
</script>

<template>
  <main class="shell">
    <header>
      <h1>AI Shepherd</h1>
      <button @click="refreshPanes">Refresh</button>
    </header>

    <nav>
      <button @click="activeTab = 'usage'; refreshUsage()">Usage Monitor</button>
      <button @click="activeTab = 'pending'">Pending Prompts</button>
      <button @click="activeTab = 'settings'">Settings</button>
    </nav>

    <section v-if="activeTab === 'usage'">
      <h2>Usage Monitor</h2>
      <p v-if="usageLoading">Loading usage...</p>
      <p v-else-if="usageSummary.length === 0">No Claude Code usage files found yet.</p>
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

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { onMounted, ref } from 'vue'
import type { PaneInfo } from './types'

const panes = ref<PaneInfo[]>([])
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

onMounted(refreshPanes)
</script>

<template>
  <main class="shell">
    <header>
      <h1>AI Shepherd</h1>
      <button @click="refreshPanes">Refresh</button>
    </header>

    <nav>
      <button @click="activeTab = 'usage'">Usage Monitor</button>
      <button @click="activeTab = 'pending'">Pending Prompts</button>
      <button @click="activeTab = 'settings'">Settings</button>
    </nav>

    <section v-if="activeTab === 'usage'">
      <h2>Usage Monitor</h2>
      <pre>{{ panes }}</pre>
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

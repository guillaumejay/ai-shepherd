<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { formatDistanceToNowStrict } from 'date-fns'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { CwdLocation, PaneInfo, SessionSummary, ShellContext, ShellType, UsageDiagnostics } from './types'

const panes = ref<PaneInfo[]>([])
const usageSummary = ref<SessionSummary[]>([])
const usageDiagnostics = ref<UsageDiagnostics | null>(null)
const usageLoading = ref(true)
const panesLoading = ref(true)
const activeTab = ref<'usage' | 'pending' | 'settings'>('usage')
const error = ref('')
const usageLastRefreshedAt = ref<string | null>(null)
const panesLastRefreshedAt = ref<string | null>(null)
let unlistenUsage: null | (() => void) = null
let usageRefreshInterval: ReturnType<typeof setInterval> | null = null

const AI_TERMINAL_PATTERNS = [/\bgemini\b/i, /\bclaude\b/i, /\bcodex\b/i, /\bopencode\b/i, /\bopen\s+code\b/i]

const isRefreshing = computed(() => usageLoading.value || panesLoading.value)
const totalTokens = computed(() => usageSummary.value.reduce((sum, item) => sum + item.tokens_used, 0))
const totalRequests = computed(() => usageSummary.value.reduce((sum, item) => sum + item.requests_used, 0))
const totalCost = computed(() => usageSummary.value.reduce((sum, item) => sum + item.cost, 0))
const providerCount = computed(() => new Set(usageSummary.value.map((item) => item.provider_id)).size)
const filteredPanes = computed(() => panes.value.filter(isAiToolPane))
const paneCount = computed(() => filteredPanes.value.length)

function compactNumber(value: number) {
  return new Intl.NumberFormat('en', {
    maximumFractionDigits: 1,
    notation: 'compact',
  }).format(value)
}

function currency(value: number) {
  return new Intl.NumberFormat('en-US', {
    currency: 'USD',
    maximumFractionDigits: 2,
    minimumFractionDigits: 2,
    style: 'currency',
  }).format(value)
}

function titleCase(value: string) {
  return value
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\b\w/g, (char) => char.toUpperCase())
}

function providerLabel(providerId: string) {
  return titleCase(providerId)
}

function shellLabel(shell: ShellType) {
  switch (shell.kind) {
    case 'power_shell':
      return 'PowerShell'
    case 'bash':
      return 'Bash'
    case 'zsh':
      return 'Zsh'
    case 'fish':
      return 'Fish'
    case 'cmd':
      return 'Cmd'
    case 'unknown':
      return titleCase(shell.value)
  }
}

function contextLabel(context: ShellContext) {
  switch (context.kind) {
    case 'native':
      return 'Local shell'
    case 'wsl':
      return `WSL ${context.value.distro}`
    case 'ssh':
      return `SSH ${context.value.host}`
  }
}

function cwdLabel(cwd: CwdLocation | null) {
  if (!cwd) {
    return 'No workspace path reported'
  }

  return cwd.normalized ?? cwd.raw
}

function isAiToolPane(pane: PaneInfo) {
  if (pane.ai_tool) {
    return true
  }

  return Boolean(fallbackAiMatch(pane))
}

function fallbackAiMatch(pane: PaneInfo) {
  const text = [pane.title, cwdLabel(pane.cwd)].join(' ')

  return AI_TERMINAL_PATTERNS.find((pattern) => pattern.test(text))?.source ?? null
}

function paneDetectionDebugLine(pane: PaneInfo) {
  const sampleStatus = pane.ai_tool_capture_error
    ? `Capture failed: ${pane.ai_tool_capture_error}`
    : `Captured ${compactNumber(pane.ai_tool_sample_chars)} chars from the pane.`

  if (pane.ai_tool) {
    return `${pane.ai_tool}: matched "${pane.ai_tool_match ?? 'unknown'}" in ${pane.ai_tool_source ?? 'unknown source'}. ${sampleStatus}`
  }

  const fallbackMatch = fallbackAiMatch(pane)

  if (fallbackMatch) {
    return `Fallback match: regex ${fallbackMatch} matched the pane title or path. ${sampleStatus}`
  }

  return `No match: title, path, and captured pane sample did not contain a known AI tool keyword. ${sampleStatus}`
}

function usageLiveTooltip() {
  return 'Usage is refreshed from recent Claude Code session files and live backend events.'
}

function usageLocalTooltip() {
  return 'Usage data is read from local session files and kept on this machine.'
}

function usageSummariesTooltip() {
  return `${compactNumber(usageSummary.value.length)} grouped session summaries are currently shown.`
}

function accountTypeTooltip(item: SessionSummary) {
  return `Account type reported for this usage summary: ${item.account_type}.`
}

function usagePeriodTooltip(item: SessionSummary) {
  return `Usage period covered by this summary: ${item.period}.`
}

function terminalAdapterTooltip(pane: PaneInfo) {
  return `Terminal integration that reported this pane: ${pane.terminal_adapter}.`
}

function aiToolTooltip(pane: PaneInfo) {
  const source = pane.ai_tool_source ?? 'an unknown source'
  const match = pane.ai_tool_match ?? 'an unknown keyword'

  return `AI coding tool detected for this pane. Matched "${match}" in ${source}.`
}

function shellTooltip(pane: PaneInfo) {
  return `Shell detected for this pane: ${shellLabel(pane.shell)}.`
}

function contextTooltip(pane: PaneInfo) {
  return `Execution context for this pane: ${contextLabel(pane.context)}.`
}

function processTooltip(pane: PaneInfo) {
  if (pane.process_name) {
    return `Foreground process reported by the terminal: ${pane.process_name}.`
  }

  return 'No foreground process name was reported by the terminal integration.'
}

function filteredPaneCopy() {
  const hiddenCount = panes.value.length - filteredPanes.value.length

  if (hiddenCount <= 0) {
    return 'Showing panes that mention Gemini, Claude, Codex, or OpenCode.'
  }

  return `Showing AI tool panes only. ${hiddenCount} other pane${hiddenCount === 1 ? '' : 's'} hidden.`
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

function relativeTimeLabel(value: string | null) {
  if (!value) {
    return 'Not synced yet'
  }

  const date = new Date(value)

  if (Number.isNaN(date.getTime())) {
    return 'Not synced yet'
  }

  return `Updated ${formatDistanceToNowStrict(date, { addSuffix: true })}`
}

function lastSeenLabel(value: string) {
  const date = new Date(value)

  if (Number.isNaN(date.getTime())) {
    return value
  }

  return `Seen ${formatDistanceToNowStrict(date, { addSuffix: true })}`
}

function openTab(tab: 'usage' | 'pending' | 'settings') {
  activeTab.value = tab

  if (tab === 'usage') {
    void refreshUsage()
  }

  if (tab === 'pending') {
    void refreshPanes()
  }
}

async function refreshPanes() {
  panesLoading.value = true

  try {
    panes.value = await invoke<PaneInfo[]>('list_panes')
    error.value = ''
    panesLastRefreshedAt.value = new Date().toISOString()
  } catch (e) {
    error.value = String(e)
    panes.value = []
  } finally {
    panesLoading.value = false
  }
}

async function refreshUsage() {
  usageLoading.value = true
  try {
    usageSummary.value = await invoke<SessionSummary[]>('get_usage_summary')
    error.value = ''
    usageLastRefreshedAt.value = new Date().toISOString()
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

onMounted(() => {
  void listen<SessionSummary[]>('usage:updated', (event) => {
    usageSummary.value = event.payload
    usageLoading.value = false
    usageLastRefreshedAt.value = new Date().toISOString()
  }).then((unlisten) => {
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
  <main class="app-shell">
    <header class="topbar">
      <div class="brand-block">
        <p class="eyebrow">Local monitor</p>
        <h1>AI Shepherd</h1>
        <p class="lede">
          A quiet instrument for tracking Claude Code usage and terminal panes without leaving your flow.
        </p>
      </div>

      <div class="status-stack">
        <span class="status-chip status-chip--info">{{ paneCount }} AI panes</span>
      </div>

      <button class="refresh-button" type="button" :disabled="isRefreshing" @click="refreshAll">
        {{ isRefreshing ? 'Refreshing' : 'Refresh' }}
      </button>
    </header>

    <nav class="tab-bar" aria-label="App sections" role="tablist">
      <button
        class="tab-button"
        type="button"
        :class="{ 'is-active': activeTab === 'usage' }"
        :aria-selected="activeTab === 'usage'"
        role="tab"
        @click="openTab('usage')"
      >
        Usage Monitor
      </button>
      <button
        class="tab-button tab-button--pending"
        type="button"
        :class="{ 'is-active': activeTab === 'pending' }"
        :aria-selected="activeTab === 'pending'"
        role="tab"
        @click="openTab('pending')"
      >
        Pending Prompts
      </button>
      <button
        class="tab-button"
        type="button"
        :class="{ 'is-active': activeTab === 'settings' }"
        :aria-selected="activeTab === 'settings'"
        role="tab"
        @click="openTab('settings')"
      >
        Settings
      </button>
    </nav>

    <section class="content" aria-live="polite">
      <section v-if="activeTab === 'usage'" class="tab-panel tab-panel--usage">
        <article class="panel panel--overview">
          <div class="panel-heading">
            <div>
              <p class="section-label">Usage summary</p>
              <h2>Current read</h2>
            </div>

            <div class="panel-heading__meta">
              <span class="mini-chip">{{ relativeTimeLabel(usageLastRefreshedAt) }}</span>
              <span class="muted-line">
                {{ usageDiagnostics ? `${usageDiagnostics.summaries} summaries tracked` : 'Diagnostics ready when needed' }}
              </span>
            </div>
          </div>

          <div v-if="usageLoading" class="state-block state-block--pending">
            <p class="state-title">Reading recent session files</p>
            <p class="panel-copy">This stays local. The app is pulling the latest usage snapshot from disk.</p>
          </div>

          <div v-else-if="usageSummary.length === 0" class="state-block state-block--empty">
            <p class="state-title">No usage summary yet</p>
            <p class="panel-copy">
              No Claude Code usage files were found. The monitor checked recent session files and found no parsed token events.
            </p>
            <dl v-if="usageDiagnostics" class="diagnostic-grid diagnostic-grid--inline">
              <div>
                <dt>Candidate files</dt>
                <dd>{{ compactNumber(usageDiagnostics.candidate_files) }}</dd>
              </div>
              <div>
                <dt>Parsed files</dt>
                <dd>{{ compactNumber(usageDiagnostics.parsed_files) }}</dd>
              </div>
              <div>
                <dt>Token events</dt>
                <dd>{{ compactNumber(usageDiagnostics.token_events) }}</dd>
              </div>
            </dl>
          </div>

          <div v-else class="overview-layout">
            <div class="overview-copy-block">
              <p class="panel-copy">
                {{ compactNumber(totalTokens) }} tokens across {{ compactNumber(totalRequests) }} requests, costing {{ currency(totalCost) }} so far.
                {{ providerCount ? `Tracking ${compactNumber(providerCount)} provider${providerCount === 1 ? '' : 's'}.` : '' }}
              </p>

              <div class="chip-row">
                <span
                  class="status-chip status-chip--live tooltip-chip"
                  :title="usageLiveTooltip()"
                  :aria-label="usageLiveTooltip()"
                >
                  Live
                </span>
                <span
                  class="status-chip status-chip--local tooltip-chip"
                  :title="usageLocalTooltip()"
                  :aria-label="usageLocalTooltip()"
                >
                  Stored locally
                </span>
                <span
                  class="status-chip status-chip--info tooltip-chip"
                  :title="usageSummariesTooltip()"
                  :aria-label="usageSummariesTooltip()"
                >
                  {{ compactNumber(usageSummary.length) }} summaries
                </span>
              </div>
            </div>

            <dl class="diagnostic-grid">
              <div>
                <dt>Tokens</dt>
                <dd>{{ compactNumber(totalTokens) }}</dd>
              </div>
              <div>
                <dt>Requests</dt>
                <dd>{{ compactNumber(totalRequests) }}</dd>
              </div>
              <div>
                <dt>Cost</dt>
                <dd>{{ currency(totalCost) }}</dd>
              </div>
            </dl>
          </div>
        </article>

        <article class="panel panel--list">
          <div class="panel-heading">
            <div>
              <p class="section-label">Session summaries</p>
              <h3>Recent providers</h3>
            </div>

            <p class="muted-line">{{ usageSummary.length }} active summary{{ usageSummary.length === 1 ? '' : 's' }}</p>
          </div>

          <ul v-if="usageSummary.length" class="summary-list">
            <li
              v-for="(item, index) in usageSummary"
              :key="`${item.provider_id}:${item.session_id ?? 'unknown'}:${index}`"
              class="summary-row"
            >
              <div class="summary-row__main">
                <div class="summary-row__topline">
                  <div class="summary-row__identity">
                    <strong class="summary-row__provider">{{ providerLabel(item.provider_id) }}</strong>
                    <p class="summary-row__title">{{ sessionLabel(item) }}</p>
                    <p v-if="item.session_id" class="summary-row__meta mono-text">
                      Session {{ item.session_id.slice(0, 12) }}
                    </p>
                  </div>

                  <div class="summary-row__badges">
                    <span
                      class="mini-chip mini-chip--local tooltip-chip"
                      :title="accountTypeTooltip(item)"
                      :aria-label="accountTypeTooltip(item)"
                    >
                      {{ item.account_type }}
                    </span>
                    <span
                      class="mini-chip mini-chip--info tooltip-chip"
                      :title="usagePeriodTooltip(item)"
                      :aria-label="usagePeriodTooltip(item)"
                    >
                      {{ item.period }}
                    </span>
                  </div>
                </div>

                <div class="summary-row__footer">
                  <dl class="summary-row__metrics">
                    <div>
                      <dt>Tokens</dt>
                      <dd>{{ compactNumber(item.tokens_used) }}</dd>
                    </div>
                    <div>
                      <dt>Requests</dt>
                      <dd>{{ compactNumber(item.requests_used) }}</dd>
                    </div>
                    <div>
                      <dt>Cost</dt>
                      <dd>{{ currency(item.cost) }}</dd>
                    </div>
                  </dl>

                  <p class="summary-row__limit mono-text">
                    <template v-if="item.token_limit !== null">Limit {{ compactNumber(item.token_limit) }}</template>
                    <template v-else>No token limit reported</template>
                  </p>
                </div>
              </div>
            </li>
          </ul>

          <div v-else-if="!usageLoading" class="state-block state-block--empty state-block--list">
            <p class="state-title">Nothing to summarize yet</p>
            <p class="panel-copy">When usage events arrive, they will appear here as compact session summaries.</p>
          </div>
        </article>
      </section>

      <section v-else-if="activeTab === 'pending'" class="tab-panel tab-panel--pending">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="section-label">Pending prompts</p>
              <h2>Open terminal panes</h2>
            </div>

            <p class="muted-line">{{ relativeTimeLabel(panesLastRefreshedAt) }}</p>
          </div>

          <div v-if="panesLoading" class="state-block state-block--pending">
            <p class="state-title">Reading terminal panes</p>
            <p class="panel-copy">Pending prompts, shell context, and workspace paths are checked locally before they appear here.</p>
          </div>

          <p v-else class="panel-copy">
            {{ filteredPaneCopy() }}
          </p>

          <ul v-if="!panesLoading && filteredPanes.length" class="pane-list">
            <li v-for="pane in filteredPanes" :key="pane.id" class="pane-row">
              <div class="pane-row__main">
                <div class="pane-row__topline">
                  <strong>{{ pane.title }}</strong>
                  <span
                    class="mini-chip mini-chip--info tooltip-chip"
                    :title="terminalAdapterTooltip(pane)"
                    :aria-label="terminalAdapterTooltip(pane)"
                  >
                    {{ pane.terminal_adapter }}
                  </span>
                  <span
                    v-if="pane.ai_tool"
                    class="mini-chip mini-chip--tool tooltip-chip"
                    :title="aiToolTooltip(pane)"
                    :aria-label="aiToolTooltip(pane)"
                  >
                    {{ pane.ai_tool }}
                  </span>
                </div>

                <div class="chip-row chip-row--tight">
                  <span
                    class="mini-chip mini-chip--info tooltip-chip"
                    :title="shellTooltip(pane)"
                    :aria-label="shellTooltip(pane)"
                  >
                    {{ shellLabel(pane.shell) }}
                  </span>
                  <span
                    class="mini-chip mini-chip--info tooltip-chip"
                    :title="contextTooltip(pane)"
                    :aria-label="contextTooltip(pane)"
                  >
                    {{ contextLabel(pane.context) }}
                  </span>
                  <span
                    class="mini-chip mini-chip--info tooltip-chip"
                    :title="processTooltip(pane)"
                    :aria-label="processTooltip(pane)"
                  >
                    {{ pane.process_name ?? 'No process name' }}
                  </span>
                </div>
              </div>

              <div class="pane-row__aside">
                <p class="mono-text pane-row__time">{{ lastSeenLabel(pane.last_seen) }}</p>
                <p class="mono-text pane-row__cwd">{{ cwdLabel(pane.cwd) }}</p>
                <p v-if="pane.cwd?.raw && pane.cwd.normalized && pane.cwd.raw !== pane.cwd.normalized" class="muted-line">
                  Raw {{ pane.cwd.raw }}
                </p>
              </div>
            </li>
          </ul>

          <div v-else class="state-block state-block--empty">
            <p class="state-title">No AI tool panes detected</p>
            <p class="panel-copy">
              Start Gemini, Claude, Codex, or OpenCode in a terminal. Matching panes will appear here with shell, workspace, and process context.
            </p>
          </div>
        </article>

      </section>

      <section v-else class="tab-panel tab-panel--settings">
        <article class="panel">
          <div class="panel-heading">
            <div>
              <p class="section-label">Settings</p>
              <h2>Local preferences</h2>
            </div>

            <span class="mini-chip mini-chip--local">No cloud sync</span>
          </div>

          <div class="settings-grid">
            <article class="settings-card">
              <h3>Current behavior</h3>
              <ul class="stack-list">
                <li>Refreshes usage and pane state on demand.</li>
                <li>Auto-refreshes usage every 10 seconds.</li>
                <li>Listens for live usage updates from the backend.</li>
                <li>Shows diagnostics when summaries are missing.</li>
              </ul>
            </article>

            <article class="settings-card settings-card--warm">
              <h3>Local trust</h3>
              <p class="panel-copy">
                Usage data stays on this machine. The interface is tuned for a quiet desktop utility beside your editor and terminal, not a management dashboard.
              </p>

              <div class="chip-row">
                <span class="status-chip status-chip--live">{{ compactNumber(usageSummary.length) }} summaries</span>
                <span class="status-chip status-chip--info">{{ compactNumber(paneCount) }} AI panes</span>
                <span class="status-chip status-chip--local">{{ usageDiagnostics?.token_events ?? 0 }} token events</span>
              </div>
            </article>

            <article class="settings-card settings-card--wide">
              <h3>Diagnostics snapshot</h3>
              <dl v-if="usageDiagnostics" class="diagnostic-grid diagnostic-grid--settings">
                <div>
                  <dt>Candidate files</dt>
                  <dd>{{ compactNumber(usageDiagnostics.candidate_files) }}</dd>
                </div>
                <div>
                  <dt>Parsed files</dt>
                  <dd>{{ compactNumber(usageDiagnostics.parsed_files) }}</dd>
                </div>
                <div>
                  <dt>Token events</dt>
                  <dd>{{ compactNumber(usageDiagnostics.token_events) }}</dd>
                </div>
                <div>
                  <dt>Summaries</dt>
                  <dd>{{ compactNumber(usageDiagnostics.summaries) }}</dd>
                </div>
              </dl>
              <p v-else class="panel-copy">Diagnostics are available after the next successful usage refresh.</p>
            </article>

            <article class="settings-card settings-card--wide">
              <h3>Pane detection debug</h3>
              <p class="panel-copy">
                This explains why a pane is classified as Gemini, Claude, Codex, OpenCode, or ignored. Backend matches come from WezTerm title, path, process name, or the captured pane sample.
              </p>

              <ul v-if="panes.length" class="debug-list">
                <li v-for="pane in panes" :key="`debug:${pane.id}`" class="debug-row" :class="{ 'debug-row--error': Boolean(pane.ai_tool_capture_error) }">
                  <div>
                    <strong>{{ pane.title || `Pane ${pane.id}` }}</strong>
                    <p class="mono-text debug-row__id">{{ pane.terminal_adapter }} pane {{ pane.id }}</p>
                  </div>

                  <p class="debug-row__reason">{{ paneDetectionDebugLine(pane) }}</p>
                  <p class="mono-text debug-row__path">{{ cwdLabel(pane.cwd) }}</p>
                </li>
              </ul>

              <p v-else class="panel-copy">No panes have been loaded yet. Open Pending Prompts or press Refresh first.</p>
            </article>
          </div>
        </article>
      </section>

      <p v-if="error" class="error-banner">{{ error }}</p>
    </section>
  </main>
</template>

<style scoped>
:global(body) {
  margin: 0;
  --color-bg: oklch(0.975 0.01 95);
  --color-bg-strong: oklch(0.955 0.012 95);
  --color-surface: oklch(0.95 0.012 95);
  --color-surface-soft: oklch(0.97 0.01 95);
  --color-border: oklch(0.82 0.014 95);
  --color-text: oklch(0.24 0.02 95);
  --color-muted: oklch(0.44 0.02 95);
  --color-moss: oklch(0.66 0.12 150);
  --color-moss-soft: oklch(0.89 0.05 150);
  --color-amber: oklch(0.72 0.12 75);
  --color-amber-soft: oklch(0.94 0.06 75);
  --color-clay: oklch(0.62 0.18 28);
  --color-clay-soft: oklch(0.93 0.02 28);
  --color-sky: oklch(0.67 0.09 235);
  --color-sky-soft: oklch(0.93 0.02 235);
  background:
    radial-gradient(circle at top left, oklch(0.99 0.012 95 / 0.98), transparent 34%),
    radial-gradient(circle at 100% 0%, oklch(0.94 0.04 150 / 0.14), transparent 26%),
    linear-gradient(180deg, var(--color-bg), var(--color-bg-strong));
  color: var(--color-text);
  font-family: 'IBM Plex Sans', 'Manrope', Inter, system-ui, sans-serif;
  -webkit-font-smoothing: antialiased;
}

:global(#app) {
  min-height: 100vh;
}

:global(*) {
  box-sizing: border-box;
}

.app-shell {
  position: relative;
  min-height: 100vh;
  padding: 24px;
  overflow: hidden;
  color: var(--color-text);
}

.app-shell::before {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(circle at 20% 0%, oklch(0.91 0.03 150 / 0.24), transparent 30%),
    radial-gradient(circle at 100% 20%, oklch(0.92 0.03 75 / 0.12), transparent 24%);
  opacity: 0.75;
}

.topbar,
.tab-bar,
.content {
  position: relative;
  z-index: 1;
}

.topbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 20px;
  align-items: start;
  margin-bottom: 16px;
}

.brand-block h1,
.panel h2,
.panel h3,
.settings-card h3 {
  margin: 0;
  font-family: 'Fraunces', 'Source Serif 4', Georgia, serif;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.brand-block h1 {
  margin-top: 2px;
  font-size: clamp(2rem, 4vw, 2.7rem);
  line-height: 1.05;
}

.eyebrow,
.section-label {
  margin: 0;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--color-muted);
}

.lede,
.panel-copy,
.muted-line {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.55;
  color: var(--color-muted);
}

.state-title {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.55;
  color: var(--color-text);
  font-weight: 700;
}

.summary-row__meta,
.summary-row__limit,
.pane-row__cwd,
.pane-row__time,
.debug-row__id,
.debug-row__path,
.debug-row__reason {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.55;
  color: var(--color-sky);
}

.status-stack {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  justify-content: flex-end;
  max-width: 320px;
}

.status-chip,
.mini-chip,
.tab-button,
.refresh-button {
  border-radius: 999px;
  border: 1px solid var(--color-border);
  font-size: 0.75rem;
  font-weight: 700;
  line-height: 1.4;
  transition:
    background-color 160ms cubic-bezier(0.25, 1, 0.5, 1),
    border-color 160ms cubic-bezier(0.25, 1, 0.5, 1),
    color 160ms cubic-bezier(0.25, 1, 0.5, 1),
    transform 160ms cubic-bezier(0.25, 1, 0.5, 1),
    box-shadow 160ms cubic-bezier(0.25, 1, 0.5, 1);
}

.status-chip,
.mini-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
}

.status-chip--live {
  background: var(--color-moss-soft);
  border-color: oklch(0.78 0.06 150);
  color: var(--color-text);
}

.status-chip--info,
.mini-chip--info {
  background: var(--color-sky-soft);
  border-color: oklch(0.82 0.03 235);
  color: var(--color-text);
}

.status-chip--soft,
.mini-chip--soft {
  background: var(--color-surface);
  color: var(--color-muted);
}

.status-chip--local,
.mini-chip--local {
  background: var(--color-moss-soft);
  border-color: oklch(0.83 0.04 150);
  color: var(--color-text);
}

.status-chip--wait,
.mini-chip--wait {
  background: var(--color-amber-soft);
  border-color: oklch(0.8 0.07 75);
  color: var(--color-text);
}

.status-chip--danger,
.mini-chip--danger {
  background: var(--color-clay-soft);
  border-color: oklch(0.76 0.12 28);
  color: var(--color-text);
}

.status-chip--info {
  background: var(--color-sky-soft);
  border-color: oklch(0.82 0.03 235);
  color: var(--color-text);
}

.mini-chip--tool {
  background: var(--color-sky-soft);
  border-color: oklch(0.82 0.03 235);
  color: var(--color-text);
}

.tooltip-chip {
  cursor: help;
}

.refresh-button {
  min-height: 40px;
  padding: 10px 14px;
  background: var(--color-moss);
  color: var(--color-text);
  box-shadow: 0 1px 2px oklch(0.24 0.02 95 / 0.06);
}

.refresh-button:hover:not(:disabled),
.refresh-button:focus-visible,
.tab-button:hover:not(.is-active),
.tab-button:focus-visible,
.summary-row:hover,
.pane-row:hover,
.settings-card:hover {
  transform: translateY(-1px);
}

.refresh-button:hover:not(:disabled),
.tab-button:hover:not(.is-active) {
  border-color: oklch(0.72 0.09 150);
  background: oklch(0.98 0.01 95);
}

.refresh-button:disabled {
  cursor: wait;
  opacity: 0.72;
}

.refresh-button:focus-visible,
.tab-button:focus-visible {
  outline: none;
  box-shadow: 0 0 0 3px oklch(0.72 0.12 150 / 0.28);
}

.tab-bar {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 6px;
  margin-bottom: 18px;
  border: 1px solid var(--color-border);
  border-radius: 16px;
  background: var(--color-surface);
  box-shadow: 0 1px 2px oklch(0.24 0.02 95 / 0.06);
}

.tab-button {
  padding: 9px 13px;
  background: transparent;
  color: var(--color-muted);
}

.tab-button.is-active {
  background: var(--color-moss-soft);
  border-color: oklch(0.78 0.06 150);
  color: var(--color-text);
}

.tab-button--pending.is-active {
  background: var(--color-amber-soft);
  border-color: oklch(0.8 0.07 75);
}

.content {
  display: grid;
  gap: 18px;
}

.tab-panel {
  display: grid;
  gap: 18px;
}

.tab-panel--usage {
  grid-template-columns: minmax(0, 1.55fr) minmax(290px, 0.95fr);
  align-items: start;
}

.tab-panel--pending,
.tab-panel--settings {
  grid-template-columns: minmax(0, 1fr);
}

.panel,
.settings-card {
  border: 1px solid var(--color-border);
  border-radius: 16px;
  background: var(--color-surface);
  box-shadow: 0 1px 2px oklch(0.24 0.02 95 / 0.06);
}

.panel {
  padding: 18px;
}

.panel--overview {
  display: grid;
  gap: 18px;
}

.panel--list {
  align-self: stretch;
}

.panel-heading {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
}

.panel-heading h2,
.panel-heading h3 {
  margin-top: 4px;
  font-size: 1.25rem;
}

.panel-heading__meta {
  display: grid;
  justify-items: end;
  gap: 6px;
  text-align: right;
}

.overview-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(220px, 0.9fr);
  gap: 16px;
  align-items: start;
}

.overview-copy-block {
  display: grid;
  gap: 14px;
}

.chip-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.chip-row--tight {
  gap: 6px;
}

.diagnostic-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.diagnostic-grid--inline {
  margin-top: 8px;
}

.diagnostic-grid--settings {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.diagnostic-grid div {
  padding: 12px;
  border-radius: 14px;
  border: 1px solid var(--color-border);
  background: var(--color-surface-soft);
}

.diagnostic-grid dt {
  margin: 0 0 6px;
  font-size: 0.74rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-muted);
}

.diagnostic-grid dd {
  margin: 0;
  font-size: 1.2rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--color-text);
}

.state-block {
  padding: 16px;
  border-radius: 14px;
  border: 1px solid var(--color-border);
  background: var(--color-surface-soft);
}

.state-block--pending {
  border-color: oklch(0.8 0.07 75);
  background: var(--color-amber-soft);
}

.state-block--empty {
  display: grid;
  gap: 8px;
}

.state-title {
  color: var(--color-text);
  font-weight: 700;
}

.summary-list,
.pane-list,
.debug-list,
.stack-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.summary-list {
  display: grid;
  gap: 12px;
}

.summary-row,
.pane-row {
  display: grid;
  gap: 14px;
  padding: 14px;
  border: 1px solid var(--color-border);
  border-radius: 14px;
  background: var(--color-surface-soft);
}

.summary-row {
  grid-template-columns: minmax(0, 1fr);
  align-items: start;
}

.pane-row {
  grid-template-columns: minmax(0, 1.25fr) minmax(220px, 0.8fr);
}

.summary-row__main,
.pane-row__main {
  display: grid;
  gap: 12px;
}

.summary-row__topline,
.pane-row__topline {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: flex-start;
  justify-content: space-between;
  min-width: 0;
}

.summary-row__identity {
  display: grid;
  gap: 4px;
  min-width: 0;
  flex: 1 1 220px;
}

.summary-row__provider,
.summary-row__title,
.summary-row__meta,
.summary-row__limit {
  min-width: 0;
}

.summary-row__provider,
.summary-row__title,
.summary-row__meta {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.summary-row__provider {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--color-sky);
}

.summary-row__title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--color-text);
}

.summary-row__badges {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  justify-content: flex-end;
}

.summary-row__footer {
  display: grid;
  gap: 10px;
}

.summary-row__metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(92px, 1fr));
  gap: 10px;
}

.summary-row__metrics div {
  display: grid;
  gap: 4px;
  align-content: start;
}

.summary-row__metrics dt,
.summary-row__metrics dd {
  margin: 0;
}

.summary-row__metrics dt {
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--color-muted);
}

.summary-row__metrics dd {
  font-size: 1rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--color-text);
}

.summary-row__limit {
  justify-self: start;
  white-space: normal;
  overflow-wrap: anywhere;
}

.pane-row__aside {
  display: grid;
  gap: 8px;
  align-content: start;
  justify-items: start;
}

.pane-row__time,
.pane-row__cwd,
.summary-row__meta,
.summary-row__limit,
.mono-text {
  font-family: 'IBM Plex Mono', 'JetBrains Mono', Consolas, monospace;
  font-size: 0.78rem;
}

.settings-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 14px;
  margin-top: 14px;
}

.settings-card {
  padding: 16px;
  display: grid;
  gap: 12px;
}

.settings-card--wide {
  grid-column: 1 / -1;
}

.settings-card--warm {
  background: linear-gradient(180deg, oklch(0.97 0.018 150), var(--color-surface));
}

.stack-list {
  display: grid;
  gap: 10px;
}

.debug-list {
  display: grid;
  gap: 10px;
}

.debug-row {
  display: grid;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: 14px;
  background: var(--color-surface-soft);
}

.debug-row strong {
  color: var(--color-text);
}

.debug-row--error {
  border-color: oklch(0.76 0.12 28);
  background: var(--color-clay-soft);
}

.debug-row--error strong,
.debug-row--error .debug-row__reason {
  color: oklch(0.48 0.12 28);
}

.debug-row__id,
.debug-row__path,
.debug-row__reason {
  margin: 0;
}

.debug-row__reason {
  color: var(--color-muted);
  font-size: 0.86rem;
  line-height: 1.5;
}

.stack-list li {
  position: relative;
  padding-left: 14px;
  color: var(--color-muted);
}

.stack-list li::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0.62em;
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--color-moss);
}

.error-banner {
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid oklch(0.65 0.18 28 / 0.55);
  background: var(--color-clay-soft);
  color: oklch(0.42 0.09 28);
  font-size: 0.875rem;
}

@media (max-width: 980px) {
  .topbar {
    grid-template-columns: minmax(0, 1fr);
  }

  .status-stack {
    justify-content: flex-start;
    max-width: none;
  }

  .tab-panel--usage,
  .settings-grid,
  .overview-layout,
  .summary-row,
  .pane-row,
  .diagnostic-grid {
    grid-template-columns: minmax(0, 1fr);
  }

}

@media (prefers-color-scheme: dark) {
  :global(body) {
    background:
      radial-gradient(circle at top left, oklch(0.22 0.02 95), transparent 34%),
      radial-gradient(circle at 100% 0%, oklch(0.34 0.04 150 / 0.22), transparent 26%),
      linear-gradient(180deg, oklch(0.18 0.02 95), oklch(0.14 0.015 95));
    color: oklch(0.9 0.01 95);
  }

  .app-shell {
    color: oklch(0.9 0.01 95);
  }

  .app-shell::before {
    background:
      radial-gradient(circle at 20% 0%, oklch(0.34 0.04 150 / 0.18), transparent 30%),
      radial-gradient(circle at 100% 20%, oklch(0.34 0.03 75 / 0.12), transparent 24%);
  }

  .eyebrow,
  .section-label,
  .lede,
  .panel-copy,
  .muted-line {
    color: oklch(0.74 0.01 95);
  }

  .state-title,
  .panel h2,
  .panel h3,
  .settings-card h3,
  .brand-block h1,
  .summary-row__metrics dd,
  .diagnostic-grid dd {
    color: oklch(0.96 0.01 95);
  }

  .summary-row__provider,
  .summary-row__meta,
  .summary-row__limit,
  .pane-row__cwd,
  .pane-row__time,
  .debug-row__id,
  .debug-row__path,
  .debug-row__reason {
    color: oklch(0.76 0.05 235);
  }

  .panel,
  .settings-card,
  .summary-row,
  .pane-row,
  .diagnostic-grid div,
  .state-block,
  .tab-bar {
    background: oklch(0.2 0.015 95);
    border-color: oklch(0.32 0.015 95);
  }

  .tab-button,
  .status-chip--soft,
  .mini-chip--soft,
  .refresh-button {
    background: oklch(0.22 0.015 95);
    border-color: oklch(0.32 0.015 95);
    color: oklch(0.9 0.01 95);
  }

  .status-chip--local,
  .mini-chip--local {
    background: oklch(0.34 0.04 150);
    border-color: oklch(0.46 0.05 150);
    color: oklch(0.96 0.01 95);
  }

  .status-chip--wait,
  .mini-chip--wait {
    background: oklch(0.38 0.08 75);
    border-color: oklch(0.54 0.1 75);
    color: oklch(0.96 0.01 95);
  }

  .status-chip--danger,
  .mini-chip--danger {
    background: oklch(0.25 0.03 28);
    border-color: oklch(0.48 0.14 28 / 0.65);
    color: oklch(0.92 0.03 28);
  }

  .tab-button.is-active,
  .status-chip--live,
  .mini-chip--info {
    color: oklch(0.96 0.01 95);
  }

  .tab-button--pending.is-active {
    background: oklch(0.38 0.08 75);
    border-color: oklch(0.54 0.1 75);
  }

  .state-block--pending {
    background: oklch(0.3 0.04 75 / 0.16);
  }

  .debug-row--error {
    border-color: oklch(0.48 0.14 28 / 0.65);
    background: oklch(0.24 0.04 28 / 0.18);
  }

  .summary-row__title,
  .diagnostic-grid dd,
  .state-title,
  .panel h2,
  .panel h3,
  .settings-card h3,
  .brand-block h1,
  .summary-row__metrics dd {
    color: oklch(0.96 0.01 95);
  }

  .error-banner {
    background: oklch(0.25 0.03 28);
    color: oklch(0.92 0.03 28);
    border-color: oklch(0.48 0.14 28 / 0.65);
  }

  .state-block--pending,
  .debug-row--error {
    background: oklch(0.3 0.04 75 / 0.16);
  }

  .debug-row--error {
    border-color: oklch(0.48 0.14 28 / 0.65);
    background: oklch(0.24 0.04 28 / 0.18);
  }
}

@media (prefers-reduced-motion: reduce) {
  .status-chip,
  .mini-chip,
  .tab-button,
  .refresh-button,
  .summary-row,
  .pane-row,
  .settings-card {
    transition: none;
  }

  .refresh-button:hover:not(:disabled),
  .tab-button:hover:not(.is-active),
  .summary-row:hover,
  .pane-row:hover,
  .settings-card:hover {
    transform: none;
  }
}
</style>

export interface CwdLocation {
  raw: string
  normalized: string | null
}

export type ShellType =
  | { kind: 'power_shell' }
  | { kind: 'bash' }
  | { kind: 'zsh' }
  | { kind: 'fish' }
  | { kind: 'cmd' }
  | { kind: 'unknown'; value: string }

export type ShellContext =
  | { kind: 'native' }
  | { kind: 'wsl'; value: { distro: string } }
  | { kind: 'ssh'; value: { host: string } }

export interface PaneInfo {
  id: string
  terminal_adapter: string
  title: string
  cwd: CwdLocation | null
  shell: ShellType
  context: ShellContext
  process_name: string | null
  ai_tool: string | null
  ai_tool_source: string | null
  ai_tool_match: string | null
  ai_tool_sample_chars: number
  ai_tool_capture_error: string | null
  last_seen: string
}

export type AccountType = 'unknown'
export type UsagePeriod = 'current'

export interface WatchDescriptor {
  path: string
}

export interface UsageEvent {
  provider_id: string
  source_file: string
  timestamp: string | null
  session_id: string | null
  session_title: string | null
  model: string | null
  input_tokens: number
  output_tokens: number
  requests_used: number
}

export interface SessionSummary {
  provider_id: string
  account_type: AccountType
  period: UsagePeriod
  session_id: string | null
  session_title: string | null
  tokens_used: number
  requests_used: number
  cost: number
  token_limit: number | null
}

export interface UsageDiagnostics {
  provider_id: string
  candidate_files: number
  parsed_files: number
  token_events: number
  summaries: number
}

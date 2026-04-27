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
  last_seen: string
}

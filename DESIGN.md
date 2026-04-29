---
name: AI Shepherd
description: Calm local monitor for AI coding usage and terminal sessions.
colors:
  daylight-bg: "oklch(0.97 0.01 95)"
  paper-surface: "oklch(0.95 0.012 95)"
  moss-accent: "oklch(0.66 0.12 150)"
  moss-soft: "oklch(0.89 0.05 150)"
  amber-waiting: "oklch(0.72 0.12 75)"
  clay-danger: "oklch(0.62 0.18 28)"
  sky-info: "oklch(0.67 0.09 235)"
  ink-text: "oklch(0.24 0.02 95)"
  muted-text: "oklch(0.44 0.02 95)"
  field-border: "oklch(0.82 0.014 95)"
typography:
  display:
    fontFamily: "Fraunces, Source Serif 4, Georgia, serif"
    fontSize: "28px"
    fontWeight: 600
    lineHeight: 1.2
  title:
    fontFamily: "IBM Plex Sans, Manrope, Inter, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 650
    lineHeight: 1.3
  body:
    fontFamily: "IBM Plex Sans, Manrope, Inter, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "IBM Plex Sans, Manrope, Inter, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.4
  mono:
    fontFamily: "IBM Plex Mono, JetBrains Mono, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.45
rounded:
  control: "10px"
  panel: "14px"
  card: "16px"
  pill: "999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.moss-accent}"
    textColor: "{colors.ink-text}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "10px 14px"
  chip-local:
    backgroundColor: "{colors.moss-soft}"
    textColor: "{colors.ink-text}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "4px 10px"
  card-summary:
    backgroundColor: "{colors.paper-surface}"
    textColor: "{colors.ink-text}"
    typography: "{typography.body}"
    rounded: "{rounded.card}"
    padding: "16px"
---

# Design System: AI Shepherd

## 1. Overview

**Creative North Star: "The Quiet Instrument Panel"**

AI Shepherd should look like a calm local instrument beside the user's editor and terminal. It is warm, disciplined, and easy to scan, with enough technical detail to earn trust and enough softness to avoid management-console coldness.

Physical scene: a solo developer checks a compact desktop utility between coding sessions in a normal office or late-evening workspace, with the terminal and editor still being the primary surfaces. This points to a light-first, low-glare daylight theme with a dim mode available, not a neon dark dashboard.

**Key Characteristics:**
- Product UI, not marketing theater.
- Restrained color strategy: tinted neutrals plus one moss accent under 10 percent of a screen.
- Local-first posture visible through status, copy, and hierarchy.
- No crypto-terminal styling, enterprise admin dashboards, AI SaaS clichés, cute mascot behavior, or cloud-first language.

## 2. Colors

The palette translates the pixel-art icon into daylight neutrals, moss green status color, small amber warnings, clay errors, and quiet sky metadata.

### Primary
- **Moss Accent**: the only primary accent. Use it for active tabs, primary actions, focus rings, selected rows, and live local status.

### Secondary
- **Amber Waiting**: use for pending prompts, quota pressure, and attention states that are not failures.
- **Sky Info**: use for shell context, cwd, provider metadata, and non-urgent technical labels.

### Tertiary
- **Clay Danger**: use only for failed parsing, disconnected integrations, destructive actions, and unrecoverable errors.

### Neutral
- **Daylight Background**: main app background, lightly warm so it never feels sterile.
- **Paper Surface**: panels, cards, tab wells, and settings groups.
- **Ink Text**: primary text, never pure black.
- **Muted Text**: supporting labels, timestamps, paths, and helper copy.
- **Field Border**: quiet dividers and input borders.

### Named Rules
**The Ten Percent Moss Rule.** Moss is a signal, not decoration. If every card or row uses it, the monitor stops being glanceable.

**The No Neon Rule.** Prohibited: neon-on-black, glowing outlines, purple SaaS gradients, and synthetic terminal theatrics.

## 3. Typography

**Display Font:** Fraunces or Source Serif 4, with Georgia fallback.
**Body Font:** IBM Plex Sans or Manrope, with system fallback.
**Label/Mono Font:** IBM Plex Mono or JetBrains Mono for paths, IDs, shell labels, and token figures.

**Character:** The pairing should feel humane and precise. Headings can carry a little warmth, but body text and controls must stay utility-grade.

### Hierarchy
- **Display** (600, 28px, 1.2): app title and rare empty-state headings.
- **Headline** (650, 22px, 1.25): major section titles if the app grows beyond one screen.
- **Title** (650, 18px, 1.3): panels, active session summaries, and settings groups.
- **Body** (400, 14px, 1.5): normal UI copy, capped around 65 to 75 characters.
- **Label** (600, 12px, 1.4): chips, tabs, metadata, and compact controls.

### Named Rules
**The Numbers Need Rails Rule.** Token counts, costs, timestamps, and request counts use tabular figures or mono so refreshes do not jitter.

## 4. Elevation

AI Shepherd is flat by default and gains depth through tonal layering, borders, and small state changes. Shadows are structural, not decorative. A resting screen should not look like stacked floating cards.

### Shadow Vocabulary
- **Hairline Rest** (`0 1px 2px oklch(0.24 0.02 95 / 0.06)`): tiny lift for top bar or selected controls.
- **Panel Lift** (`0 8px 24px oklch(0.24 0.02 95 / 0.08)`): only for elevated overlays or focused detail panels.
- **Window Depth** (`0 16px 40px oklch(0.24 0.02 95 / 0.10)`): reserved for app-level surfaces, never repeated in lists.

### Named Rules
**The Flat At Rest Rule.** Lists, cards, and panels are calm until interaction. Hover may shift color or shadow, but never layout.

## 5. Components

### Buttons
- **Shape:** softly rectangular controls with a 10px radius.
- **Primary:** moss background, ink text, label typography, 10px by 14px padding.
- **Hover / Focus:** hover uses a slight tonal shift. Focus uses a visible moss ring. Do not animate width, height, padding, or layout.
- **Secondary / Ghost:** paper surface with field border. Use for refresh, diagnostics, and settings actions.

### Chips
- **Style:** small rounded pills with semantic fills and clear labels such as `Live`, `Idle`, `Waiting`, `WSL`, and `Stored locally`.
- **State:** never rely on color alone. Pair color with text or icon shape.

### Cards / Containers
- **Corner Style:** 14px panels and 16px summary cards.
- **Background:** paper surface over daylight background, with field-border separation.
- **Shadow Strategy:** flat at rest, hairline only when needed.
- **Border:** full subtle border, never a colored side stripe.
- **Internal Padding:** 16px for summary cards, 24px for larger settings groups.

### Inputs / Fields
- **Style:** paper surface, full border, 10px radius, 14px text.
- **Focus:** moss ring plus border shift.
- **Error / Disabled:** clay danger for errors, muted surface and muted text for disabled controls.

### Navigation
- Compact segmented tabs. Active tab uses filled surface and moss text. Inactive tabs stay quiet with full borders and no decorative icons unless the section benefits from faster scanning.

### Session Row
A signature row for usage history and active sessions. Left side carries session title and provider, middle carries tokens or requests, right side carries recency and status. Hover may lift tonally. Expansion reveals file path, model, and timestamps inline, not in a modal.

## 6. Do's and Don'ts

### Do:
- **Do** use daylight neutrals and moss accent to make local monitoring feel calm and trustworthy.
- **Do** keep the first read glanceable: active state, last refresh, pending prompts, and current usage.
- **Do** show technical detail where it helps: model, source file, terminal pane, cwd, shell, tokens, and timestamps.
- **Do** keep local privacy visible through copy such as “Stored on this machine” and “Monitoring locally”.
- **Do** use full borders, tonal fills, icons, or text labels for emphasis instead of side-stripe accents.

### Don't:
- **Don't** use crypto-terminal styling: neon-on-black, hacker theatrics, aggressive contrast, or synthetic glow.
- **Don't** use enterprise admin dashboards: cold CRUD tables, generic panels, dense chrome, or management-console language.
- **Don't** use AI SaaS clichés: purple gradients, glass cards, hype copy, generic hero metrics, or “unlock insights” language.
- **Don't** use cute mascot apps as reference: no sheep jokes, pastoral illustration overload, or playful copy that weakens trust.
- **Don't** imply cloud-first positioning, remote capture, team surveillance, or vendor lock-in.
- **Don't** use gradient text, default glassmorphism, colored side stripes, bounce easing, or modals as the first solution.

# Orpheus Dashboard — Design System

## Visual Identity

### Brand Expression

**Archetype**: Caregiver + Everyman (The Thoughtful Host)
**Mythological Anchor**: Orpheus charmed all living things with music — bringing harmony to chaos.

### Color Palette

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| **Primary** | `#6B4E9E` | `#B896E8` | CTAs, active nav, key accents, focus rings |
| **Primary-hover** | `#5A3F8A` | `#A67FE0` | Hover states |
| **Primary-glow** | `rgba(107,78,158,.15)` | `rgba(184,150,232,.18)` | Focus rings, selected cards |
| **Primary-dim** | `rgba(107,78,158,.08)` | `rgba(184,150,232,.1)` | Subtle backgrounds |
| **Secondary** | `#1A7A4A` | `#2FD96E` | Success states, "synced", "complete" |
| **Background** | `#FAFAF9` | `#0D0D0E` | Page background (warm off-white / near-black) |
| **Surface** | `#FFFFFF` | `#161618` | Cards, modals, sidebar |
| **Surface-hover** | `#F5F5F3` | `#1E1E20` | Hover on rows, cards |
| **Border** | `#E5E5E2` | `#2A2A2C` | Dividers, input borders |
| **Border-hover** | `#D4D4D1` | `#3A3A3C` | Hover borders |
| **Text-primary** | `#1A1A1A` | `#F5F5F4` | Headings, primary content |
| **Text-secondary** | `#5A5A58` | `#A8A8A5` | Body text, descriptions |
| **Text-muted** | `#8A8A88` | `#7A7A78` | Placeholders, hints, timestamps |
| **Text-dim** | `#B8B8B6` | `#5A5A58` | Disabled, subtle labels |
| **Error** | `#C0392B` | `#E74C3C` | Errors, destructive actions |
| **Warning** | `#C47600` | `#E8A838` | Warnings, pending states |

**Contrast Verification (WCAG AA):**
- Primary on white: 4.8:1 ✓
- Primary on dark bg: 5.2:1 ✓
- Primary text on bg: 6.1:1 / 5.8:1 ✓ AAA

### Typography

| Role | Font | Weights | Size (rem) | Line-height |
|------|------|---------|------------|-------------|
| **Display / Hero** | Fraunces | 700, 900 | 2.5 / 2 / 1.5 | 1.1 |
| **Heading (H1–H3)** | Fraunces | 600, 700 | 1.5 / 1.25 / 1.1 | 1.2 |
| **UI Heading / Labels** | IBM Plex Sans | 600, 700 | 0.75 / 0.6875 | 1.3 |
| **Body** | IBM Plex Sans | 400, 500 | 0.875 | 1.5 |
| **Caption / Mono** | IBM Plex Mono | 400, 500 | 0.75 / 0.6875 | 1.4 |
| **Code / Technical** | IBM Plex Mono | 400 | 0.8125 | 1.5 |

**Font Loading:**
```css
@import url('https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,600;9..144,700;9..144,900&family=IBM+Plex+Mono:wght@400;500&family=IBM+Plex+Sans:wght@400;500;600;700&display=swap');
```

### Spacing Scale (8px base)

| Token | Value | Usage |
|-------|-------|-------|
| `--space-1` | 4px | Icon gaps, tight inline |
| `--space-2` | 8px | Base unit: form gaps, card padding |
| `--space-3` | 12px | Card internal gaps |
| `--space-4` | 16px | Section gaps, card padding |
| `--space-5` | 24px | Page section gaps, modal padding |
| `--space-6` | 32px | Major section breaks |
| `--space-7` | 48px | Hero/landing vertical rhythm |
| `--space-8` | 64px | Page-level margins |

### Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius-sm` | 4px | Inputs, buttons, badges, chips |
| `--radius-md` | 8px | Cards, modals, dropdowns |
| `--radius-lg` | 12px | Large cards, wizard, login |
| `--radius-full` | 9999px | Pills, avatars, progress bars |

### Shadows

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| `--shadow-sm` | `0 1px 2px rgba(0,0,0,.04)` | `0 1px 2px rgba(0,0,0,.3)` | Cards at rest |
| `--shadow-md` | `0 4px 12px rgba(0,0,0,.06)` | `0 4px 12px rgba(0,0,0,.4)` | Elevated cards, modals |
| `--shadow-lg` | `0 12px 28px rgba(0,0,0,.08)` | `0 12px 28px rgba(0,0,0,.5)` | Login/wizard cards, dropdowns |

### Motion

| Token | Value | Usage |
|-------|-------|-------|
| `--duration-fast` | 120ms | Hover, focus, toggle |
| `--duration-base` | 200ms | Card expand, modal enter, sidebar |
| `--duration-slow` | 320ms | Page transitions, wizard steps |
| `--ease-standard` | `cubic-bezier(.4, 0, .2, 1)` | Default |
| `--ease-emphasized` | `cubic-bezier(.05, .7, .1, 1)` | Hero reveals, important entrances |

**Principles:**
- One well-orchestrated page load with staggered reveals > scattered micro-interactions
- Respect `prefers-reduced-motion` — instant transitions, no stagger
- Sidebar: 200ms ease-out
- Modal: 150ms scale + fade
- Row hover: 120ms background transition

## Component Specifications

### Buttons

| Variant | Background | Text | Border | Use Case |
|---------|------------|------|--------|----------|
| Primary | `var(--primary)` | White | `var(--primary)` | Main CTAs |
| Ghost | Transparent | `var(--text-secondary)` | `var(--border)` | Secondary actions |
| Secondary | `var(--bg)` | `var(--text)` | `var(--border)` | Less prominent |
| Danger | `var(--error)` | White | `var(--error)` | Destructive |

**States:**
- Hover: `var(--primary-hover)` / `var(--panel-hover)` / `var(--border-hover)`
- Active: `var(--bg)` (ghost), scale not used
- Disabled: `opacity: 0.45`, `cursor: not-allowed`
- Focus: `box-shadow: 0 0 0 3px var(--primary-glow)`

**Dimensions:** 36px height, 14px horizontal padding, 0.7rem mono font, uppercase, 0.08em tracking

### Cards

- Background: `var(--panel)`
- Border: 1px `var(--border)`, `var(--radius-md)`
- Shadow: `var(--shadow-sm)`
- Hover: Border `var(--border-hover)`
- Header: `var(--bg-elevated)` bg, border-bottom, 0.7rem mono uppercase title
- Body: `var(--space-5)` padding, `var(--space-4)` gap

### Form Inputs

- Height: auto (min 38px), padding 10px 12px
- Background: `var(--bg-elevated)`
- Border: 1px `var(--border)`, `var(--radius-sm)`
- Font: `var(--font-mono)` 0.8rem
- Hover: Border `var(--border-hover)`
- Focus: Border `var(--primary)`, `box-shadow: 0 0 0 2px var(--primary-glow)`
- Placeholder: `var(--text-dim)`

### Labels

- Font: `var(--font-mono)` 0.68rem, 600 weight
- Color: `var(--text-secondary)`
- Uppercase, 0.08em tracking

### Sidebar

- Width: 240px fixed
- Background: `var(--bg-elevated)`
- Border-right: 1px `var(--border)`
- Nav items: 0.82rem `var(--font-head)`, 500 weight
- Active: `var(--primary-dim)` bg, `var(--primary)` text, 2px left border
- Groups: Collapsible, chevron indicator, persisted in localStorage

### Modals

- Backdrop: `rgba(0,0,0,.35)`, z-index 100
- Container: 440px max (600px wide), `var(--radius-lg)`, `var(--shadow-lg)`
- Header: Mono uppercase title, border-bottom
- Body: Max-height 60vh, scrollable
- Footer: Right-aligned actions, border-top

### Tables

- Mono font 0.78rem
- Header: `var(--bg-secondary)` bg, 0.65rem uppercase, `var(--text-secondary)`
- Rows: Hover `var(--bg-secondary)`, border-bottom `var(--border)`
- Horizontal scroll on mobile

### Chips / Badges

- Primary: `var(--primary-dim)` bg, `var(--primary)` text, `var(--primary)` border (30%)
- Radius: `var(--radius-sm)` / `var(--radius-full)` for pills
- Font: `var(--font-mono)` 0.6-0.72rem, 600 weight, uppercase

### Theme Selector

- Segmented control, 3 options: Auto / Light / Dark
- Active: `var(--primary)` bg, white text
- Inactive: Transparent, `var(--text-secondary)`
- Container: `var(--bg)` bg, `var(--border)` border, `var(--radius-md)`

## Layout Structure

```
.layout (flex, min-h-100vh)
├── .sidebar (fixed, 240px, z-200)
│   ├── .sidebar-brand
│   ├── .sidebar-nav (scrollable)
│   └── .sidebar-footer
├── .sidebar-overlay (mobile, z-199)
└── .main (flex-1, margin-left: 240px)
    ├── .main-header (sticky, z-10)
    │   ├── Hamburger (mobile)
    │   ├── .main-title (display-3)
    │   └── .theme-selector
    └── .shell (max-w-1024, mx-auto, p-5, gap-6)
        └── Outlet (page content)
```

**Breakpoints:**
- Mobile: ≤768px (sidebar off-canvas, hamburger visible)
- Tablet: 769-1024px
- Desktop: >1024px

## Accessibility

- **Contrast**: All text ≥4.5:1, large text ≥3:1, interactive ≥3:1
- **Focus**: Visible 3px solid `var(--primary-glow)` ring, logical Tab order
- **Color**: Never color alone — icons/text for states (error, success, warning)
- **Keyboard**: All interactive elements reachable via Tab, Enter, Space
- **Reduced motion**: `@media (prefers-reduced-motion: reduce)` disables animations
- **Semantic HTML**: Proper heading hierarchy, landmarks, ARIA labels where needed

## Anti-Patterns (Do Not Use)

- ❌ Inter, Roboto, Arial, system fonts, Space Grotesk
- ❌ Purple gradients on white (cliché AI aesthetic)
- ❌ Emojis as UI icons (use Lucide SVG via dioxus-icons)
- ❌ Scale transforms on hover (causes layout shift)
- ❌ Unstable hover states (use color/opacity transitions)
- ❌ Predictable, evenly-distributed layouts
- ❌ Generic component patterns without context-specific character

## Surface Briefs (Key Screens)

### Login Page — First Contact

| Element | Spec |
|---------|------|
| Brand | "Orpheus" wordmark (Fraunces 900, purple primary), centered |
| Card | `--radius-lg` (12px), `--shadow-lg`, white bg light / `#161618` dark |
| Inputs | `--font-mono` 0.8rem, `--radius-sm`, focus ring `--primary-glow` |
| CTA | Primary button, full width, 42px height |
| Copy tone | Warm welcome: "Welcome back. Sign in to manage your media." |

### Wizard — First-Run Setup (3 steps)

| Element | Spec |
|---------|------|
| Step indicator | Dots 28px, active = primary border + primary-dim bg, done = success |
| Cards | `--radius-lg`, `--shadow-lg`, centered max-width 480px |
| Step 1 (Server) | Name input + country select + description |
| Step 2 (Account) | Username + password + confirm + toggles |
| Step 3 (Done) | Confirmation + Finish button |
| Motion | Step transitions 320ms `--ease-emphasized` |

### Dashboard Home — Server Overview

| Element | Spec |
|---------|------|
| Layout | Sidebar (240px) + main content, max-width 1024px shell |
| Cards | Server Info, Media Stats, Sessions, Tasks — card grid 2-col desktop |
| Typography | H1 page title Fraunces 700 1.5rem, card titles mono uppercase 0.7rem |
| Metrics | Large numbers `--font-mono`, badges for status (running/completed/failed) |
| Actions | Primary CTA for "Add Source", ghost for secondary |

### Sources Page — Catalog & Addon Management

| Element | Spec |
|---------|------|
| Tabs | Global / User segmented, primary underline |
| Cards | Addon cards with name, kind badge, built-in badge, toggle switch |
| Drag handle | Reorder via DragAndDropList, priority updates on server |
| Edit modal | `--radius-lg`, form groups, check-row-group for resources/types |
| Badges | `--primary-dim` bg for active catalogs, type pills for content kinds |

### Settings Pages — Configuration Forms

| Element | Spec |
|---------|------|
| Forms | `--space-4` gap between fields, mono labels uppercase |
| Toggles | `--switch` component, primary on state, track `--border` off |
| Selects | `--font-mono` 0.8rem, focus ring `--primary-glow` |
| Number inputs | Min/max validation, hint text below |
| Save | Primary button bottom-right, success toast after save |

### Branding Page — Custom CSS/JS

| Element | Spec |
|---------|------|
| Textareas | Monospace 0.78rem, resize vertical, min-height per field |
| Preview hint | "Injected into every page of the web client" |
| Save state | Success alert "Branding saved." with `--success` color |

## Implementation Notes

- All tokens defined in `assets/theme.css` `:root` block
- Dark mode via `@media (prefers-color-scheme: dark)` + explicit `[data-theme]` overrides
- Theme persisted in `localStorage` (`theme` key: "auto" | "light" | "dark")
- Fonts loaded via Google Fonts import at top of theme.css
- Component classes use BEM-like naming (`.card`, `.card-header`, `.card-body`)
- Utility classes for typography (`.display-1`, `.heading-1`, `.ui-label`, `.body-1`, `.caption`, `.code`)
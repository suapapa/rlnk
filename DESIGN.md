# Design

Visual system for the rlnk GitHub Pages landing (`site/index.html`).

## Theme

- **Default:** dark terminal-inspired surface (`#080a0e` background)
- **Light:** `prefers-color-scheme: light` token overrides
- **Accent:** muted green (`#1fad5a`) for primary actions and focus; not fully saturated Tailwind green

## Color tokens

| Token | Dark | Light |
|-------|------|-------|
| `--bg` | `#080a0e` | `#f4f6f5` |
| `--surface` | `#15191d` | `#ffffff` |
| `--text` | `#f8fafc` | `#0f1412` |
| `--text-muted` | `#b8c4bf` | `#3d4a44` |
| `--accent` | `#1fad5a` | `#15803d` |
| `--line` | `#33413a` | `#c5d0ca` |

Semantic code colors: `--cyan` (GET), `--amber` (values), `--danger` (DELETE).

## Typography

- **Sans:** `system-ui` stack (no external web fonts)
- **Mono:** `ui-monospace` stack for endpoints and code blocks
- **Scale:** h1 clamp max 64px → 40px mobile; h2 34→28px; body 16px, lead 19→17px mobile
- **Wrapping:** `text-wrap: balance` on headings, `pretty` on prose

## Layout

- Max content width `--content: 1120px`
- Section padding `72px 24px` (54px 18px mobile)
- Breakpoints: `900px` (grid collapse), `680px` (header stack, full-width buttons)

## Components

- **Buttons:** 48px min-height, 8px radius, primary filled accent
- **Endpoint rows:** `<details>` accordion; chevron control 44×44px
- **Code panels:** 1px border, minimal or no shadow
- **Feature list:** stacked items with top border rhythm (no icon card grid)

## Motion

- Transitions 180ms ease on hover/focus
- `prefers-reduced-motion: reduce` disables transitions globally

## Spacing scale

4, 8, 10, 12, 14, 16, 18, 20, 24, 32, 34, 54, 72 (px)

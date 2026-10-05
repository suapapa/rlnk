# Product

## Register

brand

## Users

Backend and platform engineers who evaluate, deploy, or operate self-hosted URL shorteners. They arrive at the landing page to confirm API shape, runtime requirements, and deployment steps before integrating rlnk into their stack.

## Product Purpose

rlnk is a minimal, high-performance URL shortener API (Rust, axum, MongoDB). The landing page explains what the service does, documents the four HTTP endpoints, and shows how to run it with Docker Compose. Success means a developer can decide quickly whether rlnk fits their needs and copy working examples without reading the full README first.

## Brand Personality

**Precise, understated, trustworthy.** The interface should feel like a well-maintained open-source tool: confident without marketing hype, dark and readable by default, green accent as a signal for “go” rather than decoration.

## Anti-references

- Generic SaaS landing templates (hero metrics, identical icon-card grids, section eyebrows on every block)
- Glassmorphism headers and gradient text
- Over-rounded cards and ghost-card border-plus-shadow combos
- Loud dev-tool neon palettes that scream “AI-generated dashboard”

## Design Principles

1. **Show the API, not the pitch** — curl examples and endpoint behavior matter more than feature marketing copy.
2. **Restraint over decoration** — one accent color, system typography, flat surfaces; motion only for feedback.
3. **Copy-paste ready** — env and compose snippets must work with minimal edits.
4. **Accessible by default** — WCAG AA contrast, keyboard paths, 44px touch targets on coarse pointers.
5. **Bilingual without flash** — Korean default for local audience; English for everyone else without visible language flicker.

## Accessibility & Inclusion

- Target WCAG 2.1 AA for text contrast and focus indicators.
- Support `prefers-reduced-motion` and `prefers-color-scheme` (light/dark).
- Touch targets ≥44×44px on interactive controls used on mobile.

# Role-Differentiated UI Design (Admin Console vs Employee Portal)

**Date:** 2026-07-26
**Status:** Implemented

## Goal

Make the admin console and the employee portal visually distinct at a glance, and
modernize the whole frontend with glassmorphism, glow accents, and motion —
without changing any behavior, routes, roles, or API contracts.

## Approach

A CSS-variable theme layer scoped per shell, consumed by the shared component
classes, plus targeted restyling of the two layout shells. Two alternatives were
rejected: a full per-page redesign (too much churn/risk for no structural gain)
and a runtime theme context (unnecessary — the shell boundary *is* the theme
boundary, so a CSS class on each layout root is sufficient and zero-cost).

## Theme system (`src/index.css`)

- `:root` keeps neutral (black/gray) accent variables so unthemed surfaces
  (kiosk, forgot/reset password) are unchanged in tone.
- `.theme-admin` (on `AppLayout` root): indigo → violet accents
  (`--accent-1: #6366f1`, `--accent-2: #8b5cf6`), cool `#eef0f8` background,
  indigo glow.
- `.theme-portal` (on `PortalLayout` root): teal → emerald accents
  (`--accent-1: #0d9488`, `--accent-2: #10b981`), mint `#eff8f5` background,
  teal glow.
- Shared classes consume the variables, so every page inherits its shell's
  identity automatically: `.card` (now frosted glass), `.btn-primary` (accent
  gradient + hover glow/lift), `.btn-secondary`, `.form-input` focus ring,
  `.section-number`, `.data-table` row hover, `.spinner`.
- New primitives: `.glass-nav`, `.glass-menu`, `.glass-dark`, `.ambient-blob`,
  `.glow-dot`, `.text-gradient`, `.hover-lift`, `.shimmer`, `.scrollbar-thin`,
  keyframes `float-a/b`, `pulse-glow`, `fade-up`, `shimmer`.
- All decorative motion is disabled under `prefers-reduced-motion: reduce`;
  framer-motion components use `useReducedMotion` for the same purpose.

## Shells

**Admin console — dark "command center":** near-black sidebar with internal
aurora glow, light lockup logo, pulsing "Admin Console" chip, nav grouped into
*Workspace* / *Administration* sections, spring-animated active pill + accent
bar (framer-motion `layoutId`), gradient avatar with glow ring, dark
company-switcher dropdown (`glass-dark`), glass mobile top bar with Admin chip,
ambient indigo/violet blobs behind the content area.

**Employee portal — light "self-service":** frosted sticky header
(`glass-nav`), gradient PORTAL chip with sparkle, teal spring-animated active
nav pill, glowing gradient avatar, animated glass dropdown menus, glass mobile
bottom tab bar with a shared sliding top-indicator, ambient teal/emerald/sky
blobs.

**Shared:** `PageTransition` wraps each shell's `<Outlet />` — content fades
and rises on every route change (entrance-only, keyed by pathname, so
navigation stays snappy).

## Login & Modal

- Login: deep slate aurora background (indigo blob = admin, teal blob = portal),
  frosted white card, refined social buttons, `.form-input` fields, gradient
  submit button with dual-color glow on hover.
- Modal: frosted panel (`bg-white/95` + blur), softened dark overlay, spring
  entrance.

## Constraints honored

- No route, role, guard, or data-flow changes; all handlers/queries untouched.
- Tests assert on roles/text only — unaffected.
- No new dependencies (framer-motion and Tailwind v4 were already present).
- Deployed pages that hardcode neutral styles still read correctly against both
  themes (neutral black stays a first-class citizen of each palette).

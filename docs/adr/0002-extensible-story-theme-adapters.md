# ADR 0002: Extensible story theme adapters

## Status

Accepted on 2026-08-06.

## Context

QuotaTide originally had two story themes that presented the same quota,
pressure, and reset facts through different visual narratives. Theme selection,
labels, rendering, and persistence were enumerated independently in the
overview, settings view, Rust settings type, and a SQLite `CHECK` constraint.
Adding a theme therefore required coordinated edits across unrelated callers
and a database migration.

Story themes also need different implementation techniques. A lightweight
theme may use DOM and CSS, while a richer theme may add Web Animations or a
Canvas 2D effects layer. That implementation choice must not affect quota
calculation, alerts, collection, or the overview caller.

## Decision

- The UI exposes one `StoryCard` interface to the quota overview. It accepts a
  stable theme identifier and quota source facts.
- The story module normalizes source facts into a `StorySnapshot` before they
  reach a theme.
- A registry resolves theme identifiers to adapters. The same registry provides
  localized titles, descriptions, and lightweight settings previews. Unknown
  identifiers render the default `rising_water` adapter rather than breaking
  the overview.
- Each adapter owns its scene implementation and may use DOM, CSS, Web
  Animations, or Canvas 2D behind the seam. No shared 3D runtime is required.
- The story module owns a shared compact layout contract. Theme adapters fill
  the same summary slot and cannot change the surrounding overview grid. A
  user-controlled expanded presentation is a shared display mode available to
  every adapter; it cannot be enabled by one theme alone.
- Adapter changes remount the scene behind the `StoryCard` interface and use a
  shared geometry-stable entrance transition. Reduced-motion users receive no
  transition.
- Persisted theme identifiers are validated strings of 1-48 lowercase ASCII
  letters, digits, and underscores, starting with a letter. Schema version 16
  stores them in `story_theme_id`, initialized from the former enumerated
  `story_theme` column. The old column remains for safe application rollback.
- Story-theme code and assets remain subject to the separate budgets established
  by ADR 0001.
- Full-resolution generated masters live under `assets/story-source/`. A
  deterministic Sharp pipeline produces 2x WebP runtime assets and validates
  their dimensions, alpha channels, per-file limits, and combined size.

## Consequences

- A built-in theme adds one adapter directory containing its scene and preview,
  plus one registry entry. It does not require a database migration or new
  overview/settings branches.
- The Rust and generated TypeScript settings contracts accept forward-compatible
  theme identifiers. A version that does not contain an adapter preserves the
  identifier and renders the default theme.
- Theme-specific behavior is tested through the `StoryCard` interface, while
  persistence tests verify that a valid future identifier survives restart.
- Browser tests verify that every built-in adapter stays inside the common
  compact slot, preserves the one-screen overview contract, and responds to the
  same expanded-mode control.
- Scene and preview CSS for every built-in theme lives beside its adapter. The
  global stylesheet refers only to the shared `[data-story-theme]` contract and
  does not know adapter-specific class names.
- This decision supports bundled first-party themes. Downloadable third-party
  theme packages, signatures, and sandboxing remain out of scope.
- The third CSS-only `orbital_beacon` adapter proves that a theme can be added
  without raster assets or persistence changes. Scene-level code splitting is
  not used: after atlas optimization the complete runtime visual payload is
  small, while eager adapter availability keeps switching synchronous.

# Story theme maintainer guide

Story themes are bundled first-party adapters behind the `StoryCard` interface.
They change presentation only; quota calculation, alerts, collection, and
persistence do not depend on a theme implementation.

## Add a theme

Start with:

```sh
npm run new:story-theme -- signal_garden "信号花园" "Signal Garden"
```

The command creates a scene, preview, and colocated CSS without overwriting an
existing adapter. Then complete these steps:

1. Choose a stable identifier matching `[a-z][a-z0-9_]{0,47}`.
2. Add `Scene.tsx`, `Preview.tsx`, and their CSS under
   `ui/src/story/themes/<theme-id>/`.
3. Add one adapter entry in `ui/src/story/index.tsx` with the identifier,
   localized title and description, preview, and scene.
4. Render only the supplied `StorySnapshot`. Do not fetch data or duplicate
   quota calculations inside a scene.
5. Keep page placement out of theme CSS. The shared compact slot is owned by
   `ui/src/story/layout.css`.
6. Add interface-level tests for pressure states and any theme-specific motion.

Settings options and persistence require no new branches or database migration.
An older app preserves an unknown valid identifier and renders the default
adapter until it is upgraded.

## Rendering policy

- Prefer DOM, CSS, and Web Animations for compact scenes.
- Use Canvas 2D only for effects that would create excessive DOM nodes.
- Add a 3D runtime only when a theme requires real camera, lighting, or mesh
  interaction and the code and asset budgets have been approved explicitly.
- Respect reduced motion, increased contrast, forced colors, light/dark themes,
  200% text, and the one-screen compact overview.
- Theme selectors must remain inside the theme directory. Shared CSS may target
  `[data-story-theme]`, but not adapter-specific class names.

## Asset policy

Production UI code has a 100 KiB gzip budget and encoded visual assets have a
7 MiB gzip budget. Run:

```sh
npm run story-assets
npm run check:story-assets
npm --prefix ui run build
node scripts/qa/check-ui-budget.mjs ui/dist
```

Do not edit optimized `*-2x.webp` files by hand. Change or replace their masters
under `assets/story-source/`, then regenerate them with `npm run story-assets`.

Use one atlas per coherent animation set, crop transparent margins, size source
pixels for the largest supported device scale, and reuse CSS effects for fog,
scan lines, signals, and color states. Record generated-asset provenance beside
the theme when a new visual set is introduced.

## Verification

Run `npm --prefix ui run check`, then inspect both built-in themes at 360×460
in safe, warning, critical, and recovery states. A new adapter must preserve the
shared compact width and must not introduce horizontal or vertical overflow.
